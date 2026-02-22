import type { Meta, StoryObj } from "@storybook/react";

import { ChainProgress } from "./ChainProgress";

const meta: Meta<typeof ChainProgress> = {
  title: "Chat/ChainProgress",
  component: ChainProgress,
  parameters: {},
  decorators: [
    (Story) => (
      <div className="p-4 max-w-xl">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof ChainProgress>;

const threeModels = ["claude-3-opus", "gpt-4-turbo", "gemini-1.5-pro"];

export const FirstModel: Story = {
  args: {
    previewPosition: [0, 3],
    models: threeModels,
  },
};

export const SecondModel: Story = {
  args: {
    previewPosition: [1, 3],
    models: threeModels,
  },
};

export const LastModel: Story = {
  args: {
    previewPosition: [2, 3],
    models: threeModels,
  },
};

export const TwoModels: Story = {
  args: {
    previewPosition: [0, 2],
    models: ["claude-3-opus", "gpt-4-turbo"],
  },
};

export const TwoModelsSecond: Story = {
  args: {
    previewPosition: [1, 2],
    models: ["claude-3-opus", "gpt-4-turbo"],
  },
};

export const FourModels: Story = {
  args: {
    previewPosition: [2, 4],
    models: ["claude-3-opus", "gpt-4-turbo", "gemini-1.5-pro", "llama-3.1-70b"],
  },
};

export const WithProviderPrefix: Story = {
  args: {
    previewPosition: [1, 3],
    models: ["anthropic/claude-3-opus", "openai/gpt-4-turbo", "google/gemini-1.5-pro"],
  },
};

export const LongModelNames: Story = {
  args: {
    previewPosition: [0, 2],
    models: ["claude-3-opus-20240229", "gpt-4-turbo-preview-0125"],
  },
};
