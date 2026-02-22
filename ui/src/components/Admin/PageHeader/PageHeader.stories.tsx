import type { Meta, StoryObj } from "@storybook/react";
import { Settings } from "lucide-react";

import { PageHeader } from "./PageHeader";

const meta: Meta<typeof PageHeader> = {
  title: "Admin/PageHeader",
  component: PageHeader,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof PageHeader>;

export const Default: Story = {
  args: {
    title: "Users",
    description: "Manage users and their permissions",
    actionLabel: "New User",
    onAction: () => console.log("Action clicked"),
  },
};

export const WithoutAction: Story = {
  args: {
    title: "Dashboard",
    description: "Overview of your gateway activity",
  },
};

export const CustomIcon: Story = {
  args: {
    title: "Settings",
    description: "Configure your preferences",
    actionLabel: "Configure",
    onAction: () => console.log("Action clicked"),
    actionIcon: <Settings className="mr-2 h-4 w-4" />,
  },
};

export const Disabled: Story = {
  args: {
    title: "Projects",
    description: "Manage projects within organizations",
    actionLabel: "New Project",
    onAction: () => console.log("Action clicked"),
    actionDisabled: true,
  },
};
