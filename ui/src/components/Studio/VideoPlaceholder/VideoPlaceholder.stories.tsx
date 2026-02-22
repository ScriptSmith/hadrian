import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { VideoPlaceholder } from "./VideoPlaceholder";

const meta = {
  title: "Studio/VideoPlaceholder",
  component: VideoPlaceholder,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="h-[400px]">
        <Story />
      </div>
    ),
  ],
  args: {
    onNavigateToImages: fn(),
  },
} satisfies Meta<typeof VideoPlaceholder>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
