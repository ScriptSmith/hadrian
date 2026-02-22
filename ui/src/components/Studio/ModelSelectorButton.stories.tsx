import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { ModelSelectorButton } from "./ModelSelectorButton";
import type { ModelInfo } from "@/components/ModelPicker/model-utils";

const SAMPLE_MODELS: ModelInfo[] = [
  { id: "dall-e-3", owned_by: "openai" },
  { id: "dall-e-2", owned_by: "openai" },
  { id: "gpt-image-1", owned_by: "openai" },
  { id: "stable-diffusion-xl", owned_by: "stability" },
];

const meta = {
  title: "Studio/ModelSelectorButton",
  component: ModelSelectorButton,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[300px]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ModelSelectorButton>;

export default meta;
type Story = StoryObj<typeof ModelSelectorButton>;

function DefaultStory() {
  const [model, setModel] = useState("dall-e-3");
  return (
    <ModelSelectorButton model={model} onModelChange={setModel} availableModels={SAMPLE_MODELS} />
  );
}

export const Default: Story = {
  render: () => <DefaultStory />,
};

function DisabledStory() {
  const [model, setModel] = useState("dall-e-3");
  return (
    <ModelSelectorButton
      model={model}
      onModelChange={setModel}
      availableModels={SAMPLE_MODELS}
      disabled
    />
  );
}

export const Disabled: Story = {
  render: () => <DisabledStory />,
};

function CustomLabelStory() {
  const [model, setModel] = useState("tts-1");
  return (
    <ModelSelectorButton
      model={model}
      onModelChange={setModel}
      availableModels={[
        { id: "tts-1", owned_by: "openai" },
        { id: "tts-1-hd", owned_by: "openai" },
      ]}
      label="TTS Model"
    />
  );
}

export const CustomLabel: Story = {
  render: () => <CustomLabelStory />,
};
