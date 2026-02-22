import type { Meta, StoryObj } from "@storybook/react";

import type { MessageDebugInfo, DebugRound } from "@/components/chat-types";

import { DebugModal } from "./DebugModal";

const meta = {
  title: "Components/DebugModal",
  component: DebugModal,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => (
      <div className="h-screen">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof DebugModal>;

export default meta;
type Story = StoryObj<typeof meta>;

// Sample debug data for stories
const sampleRound1: DebugRound = {
  round: 1,
  startTime: Date.now() - 3000,
  endTime: Date.now() - 1500,
  inputItems: [{ role: "user", content: "Analyze this dataset and create a chart" }],
  requestBody: {
    model: "claude-3-opus",
    input: [{ role: "user", content: "Analyze this dataset and create a chart" }],
    tools: [
      { type: "function", function: { name: "code_interpreter" } },
      { type: "function", function: { name: "chart_render" } },
    ],
  },
  responseOutput: [
    {
      type: "message",
      role: "assistant",
      content: [{ type: "text", text: "I'll analyze the data." }],
    },
    {
      type: "function_call",
      id: "tc_123",
      call_id: "tc_123",
      name: "code_interpreter",
      arguments:
        '{"code": "import pandas as pd\\ndf = pd.read_csv(\\"data.csv\\")\\nprint(df.describe())"}',
    },
  ],
  toolCalls: [
    {
      id: "tc_123",
      name: "code_interpreter",
      arguments: {
        code: "import pandas as pd\ndf = pd.read_csv('data.csv')\nprint(df.describe())",
      },
    },
  ],
  toolResults: [
    {
      callId: "tc_123",
      toolName: "code_interpreter",
      success: true,
      output: '{"stdout": "       count  mean\\n0      100   50.5\\n"}',
    },
  ],
  continuationItems: [
    {
      type: "function_call",
      id: "tc_123",
      call_id: "tc_123",
      name: "code_interpreter",
      arguments:
        '{"code": "import pandas as pd\\ndf = pd.read_csv(\\"data.csv\\")\\nprint(df.describe())"}',
    },
    {
      type: "function_call_output",
      call_id: "tc_123",
      output: '{"stdout": "       count  mean\\n0      100   50.5\\n"}',
    },
  ],
};

const sampleRound2: DebugRound = {
  round: 2,
  startTime: Date.now() - 1500,
  endTime: Date.now() - 500,
  inputItems: [
    { role: "user", content: "Analyze this dataset and create a chart" },
    {
      type: "function_call",
      id: "tc_123",
      call_id: "tc_123",
      name: "code_interpreter",
      arguments: '{"code": "..."}',
    },
    {
      type: "function_call_output",
      call_id: "tc_123",
      output: '{"stdout": "..."}',
    },
  ],
  requestBody: {
    model: "claude-3-opus",
    input: [],
  },
  responseOutput: [
    {
      type: "message",
      role: "assistant",
      content: [
        {
          type: "text",
          text: "Based on the analysis, the data shows an average value of 50.5. Let me create a visualization.",
        },
      ],
    },
    {
      type: "function_call",
      id: "tc_456",
      call_id: "tc_456",
      name: "chart_render",
      arguments: '{"spec": {"$schema": "https://vega.github.io/schema/vega-lite/v6.json"}}',
    },
  ],
  toolCalls: [
    {
      id: "tc_456",
      name: "chart_render",
      arguments: {
        spec: {
          $schema: "https://vega.github.io/schema/vega-lite/v6.json",
          data: { values: [{ x: 1, y: 50 }] },
          mark: "bar",
          encoding: { x: { field: "x" }, y: { field: "y" } },
        },
      },
    },
  ],
  toolResults: [
    {
      callId: "tc_456",
      toolName: "chart_render",
      success: true,
      output: '{"chart_id": "chart_1"}',
    },
  ],
};

const sampleDebugInfo: MessageDebugInfo = {
  messageId: "msg_1234567890_abc123",
  model: "claude-3-opus",
  rounds: [sampleRound1, sampleRound2],
  totalDuration: 2500,
  success: true,
};

const sampleDebugInfoWithError: MessageDebugInfo = {
  messageId: "msg_error_example",
  model: "claude-3-sonnet",
  rounds: [
    {
      round: 1,
      startTime: Date.now() - 2000,
      endTime: Date.now() - 1000,
      inputItems: [{ role: "user", content: "Run this code" }],
      responseOutput: [
        {
          type: "function_call",
          id: "tc_err",
          call_id: "tc_err",
          name: "code_interpreter",
          arguments: '{"code": "import seaborn as sns"}',
        },
      ],
      toolCalls: [
        {
          id: "tc_err",
          name: "code_interpreter",
          arguments: { code: "import seaborn as sns" },
        },
      ],
      toolResults: [
        {
          callId: "tc_err",
          toolName: "code_interpreter",
          success: false,
          error: "ModuleNotFoundError: No module named 'seaborn'",
        },
      ],
    },
  ],
  totalDuration: 1000,
  success: false,
  error: "Tool execution failed",
};

const singleRoundDebugInfo: MessageDebugInfo = {
  messageId: "msg_single_round",
  model: "gpt-4",
  rounds: [
    {
      round: 1,
      startTime: Date.now() - 500,
      endTime: Date.now(),
      inputItems: [{ role: "user", content: "Hello, how are you?" }],
      responseOutput: [
        {
          type: "message",
          role: "assistant",
          content: [{ type: "text", text: "I'm doing well, thank you for asking!" }],
        },
      ],
    },
  ],
  totalDuration: 500,
  success: true,
};

export const Default: Story = {
  args: {
    debugInfo: sampleDebugInfo,
    onClose: () => console.log("Close clicked"),
  },
};

export const SingleRound: Story = {
  args: {
    debugInfo: singleRoundDebugInfo,
    onClose: () => console.log("Close clicked"),
  },
};

export const WithError: Story = {
  args: {
    debugInfo: sampleDebugInfoWithError,
    onClose: () => console.log("Close clicked"),
  },
};

export const MultipleToolCalls: Story = {
  args: {
    debugInfo: {
      ...sampleDebugInfo,
      rounds: [
        {
          ...sampleRound1,
          toolCalls: [
            ...(sampleRound1.toolCalls || []),
            {
              id: "tc_789",
              name: "sql_query",
              arguments: { query: "SELECT * FROM users LIMIT 10" },
            },
          ],
          toolResults: [
            ...(sampleRound1.toolResults || []),
            {
              callId: "tc_789",
              toolName: "sql_query",
              success: true,
              output: '{"rows": [{"id": 1, "name": "Alice"}], "columns": ["id", "name"]}',
            },
          ],
        },
        sampleRound2,
      ],
    },
    onClose: () => console.log("Close clicked"),
  },
};

export const WithSSEEvents: Story = {
  args: {
    debugInfo: {
      ...sampleDebugInfo,
      rounds: [
        {
          ...sampleRound1,
          sseEvents: [
            {
              timestamp: Date.now() - 2800,
              type: "response.created",
              data: { id: "resp_123" },
            },
            {
              timestamp: Date.now() - 2600,
              type: "response.content_part.delta",
              data: { delta: { text: "I'll" } },
            },
            {
              timestamp: Date.now() - 2400,
              type: "response.content_part.delta",
              data: { delta: { text: " analyze" } },
            },
            {
              timestamp: Date.now() - 2200,
              type: "response.tool_call.in_progress",
              data: { tool_call: { id: "tc_123", name: "code_interpreter" } },
            },
            {
              timestamp: Date.now() - 2000,
              type: "response.tool_call.done",
              data: { tool_call: { id: "tc_123", name: "code_interpreter" } },
            },
            {
              timestamp: Date.now() - 1800,
              type: "response.completed",
              data: { response: { id: "resp_123", status: "completed" } },
            },
          ],
        },
        sampleRound2,
      ],
    },
    onClose: () => console.log("Close clicked"),
  },
};
