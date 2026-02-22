import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { ToastProvider } from "@/components/Toast/Toast";
import { AudioSpeechPanel } from "./AudioSpeechPanel";
import type { ModelInfo } from "@/components/ModelPicker/model-utils";

const ttsModels: ModelInfo[] = [
  {
    id: "openai/tts-1",
    tasks: ["tts"],
    family: "tts",
    voices: ["alloy", "echo", "fable", "nova", "onyx", "shimmer"],
  },
  {
    id: "openai/gpt-4o-mini-tts",
    tasks: ["tts"],
    family: "gpt-4o-mini-tts",
    voices: [
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
    ],
  },
];

const meta = {
  title: "Studio/AudioSpeechPanel",
  component: AudioSpeechPanel,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => (
      <ToastProvider>
        <div className="h-[700px]">
          <Story />
        </div>
      </ToastProvider>
    ),
  ],
  args: {
    audioMode: "speak",
    onAudioModeChange: fn(),
    availableModels: ttsModels,
  },
} satisfies Meta<typeof AudioSpeechPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
