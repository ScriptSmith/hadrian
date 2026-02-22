import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { VoiceSelector } from "./VoiceSelector";
import type { ModelInstance } from "@/components/chat-types";
import type { ModelInfo } from "@/components/ModelPicker/model-utils";

const TTS1_VOICES = ["alloy", "echo", "fable", "nova", "onyx", "shimmer"];
const GPT4O_VOICES = [
  "alloy",
  "ash",
  "ballad",
  "coral",
  "echo",
  "fable",
  "nova",
  "onyx",
  "sage",
  "shimmer",
  "verse",
  "marin",
  "cedar",
];

const tts1Model: ModelInfo = {
  id: "openai/tts-1",
  voices: TTS1_VOICES,
  tasks: ["tts"],
};

const gpt4oTtsModel: ModelInfo = {
  id: "openai/gpt-4o-mini-tts",
  voices: GPT4O_VOICES,
  tasks: ["tts"],
};

const tts1Instance: ModelInstance = {
  id: "inst-tts1",
  modelId: "openai/tts-1",
  label: "tts-1",
  parameters: {},
};

const gpt4oInstance: ModelInstance = {
  id: "inst-gpt4o",
  modelId: "openai/gpt-4o-mini-tts",
  label: "gpt-4o-mini-tts",
  parameters: {},
};

const meta = {
  title: "Studio/VoiceSelector",
  component: VoiceSelector,
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
} satisfies Meta<typeof VoiceSelector>;

export default meta;
type Story = StoryObj<typeof VoiceSelector>;

function SingleModelStory() {
  const [voiceMap, setVoiceMap] = useState<Record<string, string[]>>({
    [tts1Instance.id]: ["alloy"],
  });
  return (
    <VoiceSelector
      instances={[tts1Instance]}
      availableModels={[tts1Model, gpt4oTtsModel]}
      voiceMap={voiceMap}
      onChange={setVoiceMap}
    />
  );
}

export const SingleModel: Story = {
  render: () => <SingleModelStory />,
};

function MultiSelectStory() {
  const [voiceMap, setVoiceMap] = useState<Record<string, string[]>>({
    [tts1Instance.id]: ["alloy", "nova"],
  });
  return (
    <VoiceSelector
      instances={[tts1Instance]}
      availableModels={[tts1Model, gpt4oTtsModel]}
      voiceMap={voiceMap}
      onChange={setVoiceMap}
    />
  );
}

export const MultiSelect: Story = {
  render: () => <MultiSelectStory />,
};

function MultiModelStory() {
  const [voiceMap, setVoiceMap] = useState<Record<string, string[]>>({
    [tts1Instance.id]: ["nova"],
    [gpt4oInstance.id]: ["coral", "sage"],
  });
  return (
    <VoiceSelector
      instances={[tts1Instance, gpt4oInstance]}
      availableModels={[tts1Model, gpt4oTtsModel]}
      voiceMap={voiceMap}
      onChange={setVoiceMap}
    />
  );
}

export const MultiModel: Story = {
  render: () => <MultiModelStory />,
};

function DisabledStory() {
  const [voiceMap, setVoiceMap] = useState<Record<string, string[]>>({
    [tts1Instance.id]: ["nova"],
  });
  return (
    <VoiceSelector
      instances={[tts1Instance]}
      availableModels={[tts1Model]}
      voiceMap={voiceMap}
      onChange={setVoiceMap}
      disabled
    />
  );
}

export const Disabled: Story = {
  render: () => <DisabledStory />,
};

export const NoModels: Story = {
  render: () => (
    <VoiceSelector instances={[]} availableModels={[]} voiceMap={{}} onChange={() => {}} />
  ),
};
