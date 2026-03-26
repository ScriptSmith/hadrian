import type { Meta, StoryObj } from "@storybook/react";

import { PageNotice } from "./PageNotice";

const meta: Meta<typeof PageNotice> = {
  title: "Components/PageNotice",
  component: PageNotice,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof PageNotice>;

export const Default: Story = {
  args: {
    title: "Knowledge Bases",
    message: "This page is currently unavailable.",
  },
};

export const CustomMessage: Story = {
  args: {
    title: "API Keys",
    message:
      "API key management has been disabled by your administrator. Contact support for assistance.",
  },
};
