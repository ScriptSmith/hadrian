import type { Meta, StoryObj } from "@storybook/react";

import { TooltipProvider } from "@/components/Tooltip/Tooltip";

import { ModelDetailsPanel } from "./ModelDetailsPanel";
import type { ModelInfo } from "./model-utils";

const meta = {
  title: "Components/ModelPicker/ModelDetailsPanel",
  component: ModelDetailsPanel,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <TooltipProvider>
        <div className="w-[280px] h-[500px] border rounded-lg bg-popover">
          <Story />
        </div>
      </TooltipProvider>
    ),
  ],
} satisfies Meta<typeof ModelDetailsPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

const sampleModel: ModelInfo = {
  id: "openai/gpt-4o",
  owned_by: "openai",
  context_length: 128000,
  max_output_tokens: 16384,
  capabilities: {
    vision: true,
    reasoning: false,
    tool_call: true,
    structured_output: true,
    temperature: true,
  },
  catalog_pricing: { input: 5.0, output: 15.0 },
  family: "gpt-4o",
  knowledge_cutoff: "2024-10",
  release_date: "2024-05-13",
  description: "GPT-4o is OpenAI's most advanced multimodal model with vision capabilities.",
};

const reasoningModel: ModelInfo = {
  id: "openai/o1-preview",
  owned_by: "openai",
  context_length: 128000,
  max_output_tokens: 32768,
  capabilities: {
    vision: false,
    reasoning: true,
    tool_call: false,
    structured_output: false,
    temperature: false,
  },
  catalog_pricing: { input: 15.0, output: 60.0, reasoning: 60.0 },
  family: "o1",
  knowledge_cutoff: "2024-10",
  release_date: "2024-09-12",
  description: "OpenAI's reasoning model that thinks step-by-step before responding.",
};

const openWeightsModel: ModelInfo = {
  id: "deepseek/deepseek-chat",
  owned_by: "deepseek",
  context_length: 64000,
  max_output_tokens: 8192,
  capabilities: {
    vision: false,
    reasoning: true,
    tool_call: true,
    structured_output: false,
    temperature: true,
  },
  catalog_pricing: { input: 0.14, output: 0.28 },
  open_weights: true,
  family: "deepseek-v3",
  knowledge_cutoff: "2024-07",
  description: "DeepSeek-V3 with MoE architecture and advanced reasoning capabilities.",
};

const freeModel: ModelInfo = {
  id: "google/gemini-2.0-flash-exp",
  owned_by: "google",
  context_length: 1000000,
  max_output_tokens: 8192,
  capabilities: {
    vision: true,
    reasoning: false,
    tool_call: true,
    structured_output: true,
    temperature: true,
  },
  catalog_pricing: { input: 0, output: 0 },
  family: "gemini-2.0",
  release_date: "2024-12",
};

const minimalModel: ModelInfo = {
  id: "mistral/mistral-small-latest",
  owned_by: "mistral",
  context_length: 32000,
};

export const Default: Story = {
  args: {
    model: sampleModel,
  },
};

export const ReasoningModel: Story = {
  args: {
    model: reasoningModel,
  },
};

export const OpenWeightsModel: Story = {
  args: {
    model: openWeightsModel,
  },
};

export const FreeModel: Story = {
  args: {
    model: freeModel,
  },
};

export const MinimalInfo: Story = {
  args: {
    model: minimalModel,
  },
};

export const WithCloseButton: Story = {
  args: {
    model: sampleModel,
    onClose: () => alert("Close clicked"),
  },
};

export const NoModel: Story = {
  args: {
    model: null,
  },
};
