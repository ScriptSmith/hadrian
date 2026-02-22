import type { Meta, StoryObj } from "@storybook/react";
import { AlphaBanner } from "./AlphaBanner";

const meta: Meta<typeof AlphaBanner> = {
  title: "UI/AlphaBanner",
  component: AlphaBanner,
  parameters: {
    layout: "fullscreen",
  },
  decorators: [
    (Story) => {
      localStorage.removeItem("hadrian-alpha-banner-dismissed");
      return <Story />;
    },
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
