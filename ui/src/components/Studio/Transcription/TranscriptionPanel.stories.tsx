import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { ToastProvider } from "@/components/Toast/Toast";
import { TranscriptionPanel } from "./TranscriptionPanel";

const meta = {
  title: "Studio/TranscriptionPanel",
  component: TranscriptionPanel,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => (
      <ToastProvider>
        <div className="h-[600px]">
          <Story />
        </div>
      </ToastProvider>
    ),
  ],
  args: {
    onAudioModeChange: fn(),
  },
} satisfies Meta<typeof TranscriptionPanel>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Transcribe: Story = {
  args: {
    mode: "transcribe",
    audioMode: "transcribe",
  },
};

export const Translate: Story = {
  args: {
    mode: "translate",
    audioMode: "translate",
    chatModels: [
      { id: "openai/gpt-4o-mini", owned_by: "openai" },
      { id: "openai/gpt-4o", owned_by: "openai" },
      { id: "anthropic/claude-sonnet-4-5-20250929", owned_by: "anthropic" },
    ],
  },
};
