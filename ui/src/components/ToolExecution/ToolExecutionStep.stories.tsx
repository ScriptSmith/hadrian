import type { Meta, StoryObj } from "@storybook/react";
import { ToolExecutionStep } from "./ToolExecutionStep";
import type { ToolExecution, Artifact } from "@/components/chat-types";

const meta = {
  title: "Chat/ToolExecution/ToolExecutionStep",
  component: ToolExecutionStep,
  parameters: {
    layout: "padded",
  },
} satisfies Meta<typeof ToolExecutionStep>;

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
      { name: "Total", value: 1250 },
      { name: "Average", value: 250 },
    ],
  },
  role: "output",
});

const makeExecution = (overrides: Partial<ToolExecution>): ToolExecution => ({
  id: `exec-${Math.random().toString(36).slice(2)}`,
  toolName: "code_interpreter",
  status: "success",
  startTime: Date.now() - 1000,
  endTime: Date.now(),
  duration: 1000,
  input: {},
  inputArtifacts: [],
  outputArtifacts: [],
  round: 1,
  ...overrides,
});

export const Success: Story = {
  args: {
    execution: makeExecution({
      toolName: "code_interpreter",
      status: "success",
      duration: 1234,
      inputArtifacts: [makeCodeArtifact("data = [1, 2, 3, 4, 5]\nprint(sum(data))")],
      outputArtifacts: [makeTableArtifact()],
    }),
  },
};

export const Running: Story = {
  args: {
    execution: makeExecution({
      toolName: "code_interpreter",
      status: "running",
      endTime: undefined,
      duration: undefined,
      inputArtifacts: [makeCodeArtifact("# Processing large dataset...")],
      statusMessage: "Executing code...",
    }),
  },
};

export const Error: Story = {
  args: {
    execution: makeExecution({
      toolName: "code_interpreter",
      status: "error",
      duration: 150,
      inputArtifacts: [makeCodeArtifact("undefined_variable")],
      error: "NameError: name 'undefined_variable' is not defined",
    }),
  },
};

export const Pending: Story = {
  args: {
    execution: makeExecution({
      toolName: "sql_query",
      status: "pending",
      startTime: Date.now(),
      endTime: undefined,
      duration: undefined,
    }),
  },
};

export const SqlQuery: Story = {
  args: {
    execution: makeExecution({
      toolName: "sql_query",
      status: "success",
      duration: 456,
      outputArtifacts: [makeTableArtifact()],
    }),
  },
};

export const FileSearch: Story = {
  args: {
    execution: makeExecution({
      toolName: "file_search",
      status: "success",
      duration: 234,
    }),
  },
};

export const LongCode: Story = {
  args: {
    execution: makeExecution({
      toolName: "code_interpreter",
      status: "success",
      duration: 2500,
      inputArtifacts: [
        makeCodeArtifact(
          `import pandas as pd
import numpy as np
import matplotlib.pyplot as plt

# Load and process data
df = pd.read_csv('data.csv')
df['normalized'] = (df['value'] - df['value'].mean()) / df['value'].std()

# Create visualization
fig, axes = plt.subplots(2, 2, figsize=(12, 10))
# ... more code here`
        ),
      ],
    }),
    defaultInputExpanded: false,
  },
};
