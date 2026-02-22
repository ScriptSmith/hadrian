import type { Meta, StoryObj } from "@storybook/react";
import { MultiModelResultGrid } from "./MultiModelResultGrid";
import type { InstanceResult } from "@/pages/studio/useMultiModelExecution";

const meta = {
  title: "Studio/MultiModelResultGrid",
  component: MultiModelResultGrid,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[800px]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof MultiModelResultGrid<string>>;

export default meta;
type Story = StoryObj<typeof meta>;

function makeResult(
  id: string,
  modelId: string,
  status: "loading" | "complete" | "error",
  data?: string,
  error?: string,
  label?: string,
  costMicrocents?: number
): [string, InstanceResult<string>] {
  return [
    id,
    {
      instanceId: id,
      modelId,
      label,
      status,
      data,
      error,
      durationMs: status === "complete" ? 1234 : undefined,
      costMicrocents,
    },
  ];
}

export const SingleModel: Story = {
  args: {
    results: new Map([makeResult("gpt-4o", "gpt-4o", "complete", "Hello from GPT-4o!")]),
    renderResult: (r: InstanceResult<string>) => (
      <div className="rounded-lg border p-4 text-sm">{r.data}</div>
    ),
  },
};

export const TwoModels: Story = {
  args: {
    results: new Map([
      makeResult("gpt-4o", "gpt-4o", "complete", "Response from GPT-4o"),
      makeResult("claude-3", "claude-3", "complete", "Response from Claude 3"),
    ]),
    renderResult: (r: InstanceResult<string>) => (
      <div className="rounded-lg border p-4 text-sm">{r.data}</div>
    ),
  },
};

export const ThreeModels: Story = {
  args: {
    results: new Map([
      makeResult("gpt-4o", "gpt-4o", "complete", "Response A"),
      makeResult("claude-3", "claude-3", "complete", "Response B"),
      makeResult("gemini", "gemini-pro", "complete", "Response C", undefined, "Gemini Pro"),
    ]),
    renderResult: (r: InstanceResult<string>) => (
      <div className="rounded-lg border p-4 text-sm">{r.data}</div>
    ),
  },
};

export const Loading: Story = {
  args: {
    results: new Map([
      makeResult("gpt-4o", "gpt-4o", "loading"),
      makeResult("claude-3", "claude-3", "loading"),
    ]),
    renderResult: (r: InstanceResult<string>) => (
      <div className="rounded-lg border p-4 text-sm">{r.data}</div>
    ),
  },
};

export const MixedStates: Story = {
  args: {
    results: new Map([
      makeResult("gpt-4o", "gpt-4o", "complete", "Finished!"),
      makeResult("claude-3", "claude-3", "loading"),
      makeResult("gemini", "gemini-pro", "error", undefined, "Rate limit exceeded"),
    ]),
    renderResult: (r: InstanceResult<string>) => (
      <div className="rounded-lg border p-4 text-sm">{r.data}</div>
    ),
  },
};

export const WithError: Story = {
  args: {
    results: new Map([makeResult("gpt-4o", "gpt-4o", "error", undefined, "Model not available")]),
    renderResult: (r: InstanceResult<string>) => (
      <div className="rounded-lg border p-4 text-sm">{r.data}</div>
    ),
  },
};

export const WithCost: Story = {
  args: {
    results: new Map([
      makeResult("gpt-4o", "gpt-4o", "complete", "Response A", undefined, undefined, 2500),
      makeResult("claude-3", "claude-3", "complete", "Response B", undefined, undefined, 18000),
    ]),
    renderResult: (r: InstanceResult<string>) => (
      <div className="rounded-lg border p-4 text-sm">{r.data}</div>
    ),
  },
};

export const SingleModelWithCost: Story = {
  args: {
    results: new Map([
      makeResult(
        "dall-e-3",
        "dall-e-3",
        "complete",
        "Generated image placeholder",
        undefined,
        undefined,
        40000
      ),
    ]),
    renderResult: (r: InstanceResult<string>) => (
      <div className="rounded-lg border p-4 text-sm">{r.data}</div>
    ),
  },
};
