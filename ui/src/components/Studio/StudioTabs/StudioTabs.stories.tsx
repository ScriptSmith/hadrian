import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "storybook/test";
import { StudioTabs } from "./StudioTabs";

const meta = {
  title: "Studio/StudioTabs",
  component: StudioTabs,
  parameters: {
    layout: "centered",
    a11y: {
      config: {
        rules: [
          // Tab panels don't exist in isolation - aria-controls references are valid in context
          { id: "aria-valid-attr-value", enabled: false },
        ],
      },
    },
  },
  decorators: [
    (Story) => (
      <div className="w-[600px]">
        <Story />
      </div>
    ),
  ],
  args: {
    onTabChange: fn(),
  },
} satisfies Meta<typeof StudioTabs>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    activeTab: "images",
  },
};

export const AudioTab: Story = {
  args: {
    activeTab: "audio",
  },
};

export const VideoTab: Story = {
  args: {
    activeTab: "video",
  },
};
