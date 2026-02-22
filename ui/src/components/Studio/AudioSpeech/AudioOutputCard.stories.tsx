import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { AudioOutputCard } from "./AudioOutputCard";
import type { AudioHistoryEntry } from "@/pages/studio/types";

const meta = {
  title: "Studio/AudioOutputCard",
  component: AudioOutputCard,
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
  args: {
    onDelete: fn(),
  },
} satisfies Meta<typeof AudioOutputCard>;

export default meta;
type Story = StoryObj<typeof meta>;

const sampleEntry: AudioHistoryEntry = {
  id: "audio-1",
  text: "Hello, welcome to the demo. This is a sample audio output.",
  options: { speed: 1.0, format: "mp3" },
  results: [
    {
      instanceId: "tts-1::nova",
      modelId: "tts-1",
      voice: "nova",
      audioData: "",
    },
  ],
  createdAt: Date.now(),
};

export const Default: Story = {
  args: {
    entry: sampleEntry,
  },
};

export const MultiVoice: Story = {
  args: {
    entry: {
      ...sampleEntry,
      id: "audio-multi-voice",
      results: [
        {
          instanceId: "tts-1::alloy",
          modelId: "tts-1",
          label: "tts-1 — alloy",
          voice: "alloy",
          audioData: "",
        },
        {
          instanceId: "tts-1::nova",
          modelId: "tts-1",
          label: "tts-1 — nova",
          voice: "nova",
          audioData: "",
        },
      ],
    },
  },
};

export const MultiModel: Story = {
  args: {
    entry: {
      ...sampleEntry,
      id: "audio-multi",
      results: [
        {
          instanceId: "tts-1::alloy",
          modelId: "tts-1",
          label: "tts-1 — alloy",
          voice: "alloy",
          audioData: "",
          costMicrocents: 15000,
        },
        {
          instanceId: "gpt-4o-mini-tts::coral",
          modelId: "gpt-4o-mini-tts",
          label: "gpt-4o-mini-tts — coral",
          voice: "coral",
          audioData: "",
          costMicrocents: 24000,
        },
      ],
    },
  },
};

export const LongText: Story = {
  args: {
    entry: {
      ...sampleEntry,
      text: "This is a longer text that demonstrates how the audio output card handles multi-line content. The text preview will be clamped to two lines with an ellipsis for overflow content.",
    },
  },
};
