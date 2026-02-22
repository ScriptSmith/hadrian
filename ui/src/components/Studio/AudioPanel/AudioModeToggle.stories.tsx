import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { AudioModeToggle } from "./AudioModeToggle";

const meta = {
  title: "Studio/AudioModeToggle",
  component: AudioModeToggle,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[400px]">
        <Story />
      </div>
    ),
  ],
  args: {
    onChange: fn(),
  },
} satisfies Meta<typeof AudioModeToggle>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Speak: Story = {
  args: {
    value: "speak",
  },
};

export const Transcribe: Story = {
  args: {
    value: "transcribe",
  },
};

export const Translate: Story = {
  args: {
    value: "translate",
  },
};
