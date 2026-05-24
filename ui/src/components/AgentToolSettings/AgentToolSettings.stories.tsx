import type { Meta, StoryObj } from "@storybook/react";
import { AgentToolSettings } from "./AgentToolSettings";

const meta: Meta<typeof AgentToolSettings> = {
  title: "Chat/AgentToolSettings",
  component: AgentToolSettings,
  decorators: [
    (Story) => (
      <div className="p-4 w-72">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof AgentToolSettings>;

export const Default: Story = {};

export const Disabled: Story = {
  args: { disabled: true },
};
