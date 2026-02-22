import type { Meta, StoryObj } from "@storybook/react";

import { ToolCallIndicator, type ToolCall } from "./ToolCallIndicator";

const meta: Meta<typeof ToolCallIndicator> = {
  title: "Components/ToolCallIndicator",
  component: ToolCallIndicator,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[500px]">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof ToolCallIndicator>;

// Single tool call states
export const FileSearchExecuting: Story = {
  args: {
    toolCalls: [{ id: "1", type: "file_search", status: "executing" }],
  },
};

export const FileSearchPending: Story = {
  args: {
    toolCalls: [{ id: "1", type: "file_search", status: "pending" }],
  },
};

export const FileSearchCompleted: Story = {
  args: {
    toolCalls: [{ id: "1", type: "file_search", status: "completed" }],
  },
};

export const FileSearchFailed: Story = {
  args: {
    toolCalls: [{ id: "1", type: "file_search", status: "failed", error: "timeout" }],
  },
};

export const WebSearchExecuting: Story = {
  args: {
    toolCalls: [{ id: "1", type: "web_search", status: "executing" }],
  },
};

export const CodeInterpreterExecuting: Story = {
  args: {
    toolCalls: [{ id: "1", type: "code_interpreter", status: "executing" }],
  },
};

export const FunctionCallExecuting: Story = {
  args: {
    toolCalls: [{ id: "1", type: "function", name: "get_weather", status: "executing" }],
  },
};

// Multiple tool calls
export const MultipleToolCalls: Story = {
  args: {
    toolCalls: [
      { id: "1", type: "file_search", status: "completed" },
      { id: "2", type: "web_search", status: "executing" },
      { id: "3", type: "code_interpreter", status: "pending" },
    ] satisfies ToolCall[],
  },
};

export const AllExecuting: Story = {
  args: {
    toolCalls: [
      { id: "1", type: "file_search", status: "executing" },
      { id: "2", type: "web_search", status: "executing" },
    ] satisfies ToolCall[],
  },
};

export const MixedWithFailure: Story = {
  args: {
    toolCalls: [
      { id: "1", type: "file_search", status: "completed" },
      { id: "2", type: "web_search", status: "failed", error: "rate limit" },
      { id: "3", type: "function", name: "analyze", status: "executing" },
    ] satisfies ToolCall[],
  },
};

// Empty state
export const Empty: Story = {
  args: {
    toolCalls: [],
  },
};
