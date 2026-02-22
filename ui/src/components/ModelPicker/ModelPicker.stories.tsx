import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";

import { Button } from "@/components/Button/Button";
import { TooltipProvider } from "@/components/Tooltip/Tooltip";
import { PreferencesProvider } from "@/preferences/PreferencesProvider";

import { ModelPicker, type ModelInfo } from "./ModelPicker";

const meta: Meta<typeof ModelPicker> = {
  title: "Chat/ModelPicker",
  component: ModelPicker,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <PreferencesProvider>
        <TooltipProvider>
          <Story />
        </TooltipProvider>
      </PreferencesProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

const mockModels: ModelInfo[] = [
  // Anthropic
  {
    id: "anthropic/claude-3-5-sonnet-20241022",
    owned_by: "anthropic",
    context_length: 200000,
    max_output_tokens: 8192,
    pricing: { prompt: "0.000003", completion: "0.000015" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    catalog_pricing: { input: 3.0, output: 15.0 },
    family: "claude-3.5-sonnet",
    knowledge_cutoff: "2024-04",
    release_date: "2024-10-22",
    description: "Claude 3.5 Sonnet combines high performance with fast response times.",
  },
  {
    id: "anthropic/claude-3-5-haiku-20241022",
    owned_by: "anthropic",
    context_length: 200000,
    max_output_tokens: 8192,
    pricing: { prompt: "0.0000008", completion: "0.000004" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    catalog_pricing: { input: 0.8, output: 4.0 },
    family: "claude-3.5-haiku",
    knowledge_cutoff: "2024-04",
  },
  {
    id: "anthropic/claude-3-opus-20240229",
    owned_by: "anthropic",
    context_length: 200000,
    max_output_tokens: 4096,
    pricing: { prompt: "0.000015", completion: "0.000075" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    catalog_pricing: { input: 15.0, output: 75.0 },
    family: "claude-3-opus",
    knowledge_cutoff: "2024-01",
    release_date: "2024-02-29",
    description: "Claude 3 Opus is Anthropic's most capable model for complex tasks.",
  },
  // OpenAI
  {
    id: "openai/gpt-4o",
    owned_by: "openai",
    context_length: 128000,
    max_output_tokens: 16384,
    pricing: { prompt: "0.000005", completion: "0.000015" },
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
  },
  {
    id: "openai/gpt-4o-mini",
    owned_by: "openai",
    context_length: 128000,
    max_output_tokens: 16384,
    pricing: { prompt: "0.00000015", completion: "0.0000006" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 0.15, output: 0.6 },
    family: "gpt-4o",
    knowledge_cutoff: "2024-10",
  },
  {
    id: "openai/gpt-4-turbo",
    owned_by: "openai",
    context_length: 128000,
    max_output_tokens: 4096,
    pricing: { prompt: "0.00001", completion: "0.00003" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 10.0, output: 30.0 },
    family: "gpt-4-turbo",
    knowledge_cutoff: "2023-12",
  },
  {
    id: "openai/o1-preview",
    owned_by: "openai",
    context_length: 128000,
    max_output_tokens: 32768,
    pricing: { prompt: "0.000015", completion: "0.00006" },
    capabilities: {
      vision: false,
      reasoning: true,
      tool_call: false,
      structured_output: false,
      temperature: false,
    },
    catalog_pricing: { input: 15.0, output: 60.0 },
    family: "o1",
    knowledge_cutoff: "2024-10",
    release_date: "2024-09-12",
    description: "OpenAI's reasoning model that thinks step-by-step before responding.",
  },
  // Google
  {
    id: "google/gemini-2.0-flash-exp",
    owned_by: "google",
    context_length: 1000000,
    max_output_tokens: 8192,
    pricing: { prompt: "0", completion: "0" },
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
  },
  {
    id: "google/gemini-1.5-pro",
    owned_by: "google",
    context_length: 2000000,
    max_output_tokens: 8192,
    pricing: { prompt: "0.00000125", completion: "0.000005" },
    capabilities: {
      vision: true,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 1.25, output: 5.0 },
    family: "gemini-1.5",
    knowledge_cutoff: "2024-05",
  },
  // Mistral
  {
    id: "mistral/mistral-large-latest",
    owned_by: "mistral",
    context_length: 128000,
    pricing: { prompt: "0.000002", completion: "0.000006" },
    capabilities: {
      vision: false,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 2.0, output: 6.0 },
    family: "mistral-large",
  },
  {
    id: "mistral/mistral-small-latest",
    owned_by: "mistral",
    context_length: 32000,
    pricing: { prompt: "0.0000002", completion: "0.0000006" },
    capabilities: {
      vision: false,
      reasoning: false,
      tool_call: true,
      structured_output: true,
      temperature: true,
    },
    catalog_pricing: { input: 0.2, output: 0.6 },
    family: "mistral-small",
  },
  // DeepSeek
  {
    id: "deepseek/deepseek-chat",
    owned_by: "deepseek",
    context_length: 64000,
    max_output_tokens: 8192,
    pricing: { prompt: "0.00000014", completion: "0.00000028" },
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
  },
  // Meta
  {
    id: "meta/llama-3.3-70b-instruct",
    owned_by: "meta",
    context_length: 128000,
    max_output_tokens: 4096,
    capabilities: {
      vision: false,
      reasoning: false,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    open_weights: true,
    family: "llama-3.3",
    knowledge_cutoff: "2024-03",
    release_date: "2024-12-06",
  },
  // Cohere
  {
    id: "cohere/command-r-plus",
    owned_by: "cohere",
    context_length: 128000,
    pricing: { prompt: "0.0000025", completion: "0.00001" },
    capabilities: {
      vision: false,
      reasoning: false,
      tool_call: true,
      structured_output: false,
      temperature: true,
    },
    catalog_pricing: { input: 2.5, output: 10.0 },
    family: "command-r",
  },
  // Qwen
  {
    id: "qwen/qwen-2.5-coder-32b-instruct",
    owned_by: "qwen",
    context_length: 32000,
    capabilities: {
      vision: false,
      reasoning: false,
      tool_call: false,
      structured_output: false,
      temperature: true,
    },
    open_weights: true,
    family: "qwen-2.5",
    description:
      "Specialized coding model from the Qwen team with strong code generation abilities.",
  },
];

function DefaultStory() {
  const [open, setOpen] = useState(false);
  const [selectedModels, setSelectedModels] = useState<string[]>([]);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Select Models ({selectedModels.length})</Button>
      <ModelPicker
        open={open}
        onClose={() => setOpen(false)}
        selectedModels={selectedModels}
        onModelsChange={setSelectedModels}
        availableModels={mockModels}
      />
    </>
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function WithPreselectedStory() {
  const [open, setOpen] = useState(false);
  const [selectedModels, setSelectedModels] = useState<string[]>([
    "anthropic/claude-3-5-sonnet-20241022",
    "openai/gpt-4o",
  ]);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Select Models ({selectedModels.length})</Button>
      <ModelPicker
        open={open}
        onClose={() => setOpen(false)}
        selectedModels={selectedModels}
        onModelsChange={setSelectedModels}
        availableModels={mockModels}
      />
    </>
  );
}

export const WithPreselected: Story = {
  render: () => <WithPreselectedStory />,
};

function LimitedSelectionStory() {
  const [open, setOpen] = useState(false);
  const [selectedModels, setSelectedModels] = useState<string[]>([]);

  return (
    <>
      <Button onClick={() => setOpen(true)}>Select Models (Max 3)</Button>
      <ModelPicker
        open={open}
        onClose={() => setOpen(false)}
        selectedModels={selectedModels}
        onModelsChange={setSelectedModels}
        availableModels={mockModels}
        maxModels={3}
      />
    </>
  );
}

export const LimitedSelection: Story = {
  render: () => <LimitedSelectionStory />,
};

function OpenByDefaultStory() {
  const [open, setOpen] = useState(true);
  const [selectedModels, setSelectedModels] = useState<string[]>([]);

  return (
    <ModelPicker
      open={open}
      onClose={() => setOpen(false)}
      selectedModels={selectedModels}
      onModelsChange={setSelectedModels}
      availableModels={mockModels}
    />
  );
}

export const OpenByDefault: Story = {
  render: () => <OpenByDefaultStory />,
};
