import type { Meta, StoryObj } from "@storybook/react";

import { DetailPageHeader } from "./DetailPageHeader";

const meta: Meta<typeof DetailPageHeader> = {
  title: "Admin/DetailPageHeader",
  component: DetailPageHeader,
  parameters: {
    layout: "padded",
  },
};

export default meta;
type Story = StoryObj<typeof DetailPageHeader>;

export const Default: Story = {
  args: {
    title: "Acme Corporation",
    slug: "acme-corp",
    createdAt: "2024-01-15T10:30:00Z",
    onBack: () => console.log("Back clicked"),
    onEdit: () => console.log("Edit clicked"),
  },
};

export const WithoutEdit: Story = {
  args: {
    title: "Acme Corporation",
    slug: "acme-corp",
    createdAt: "2024-01-15T10:30:00Z",
    onBack: () => console.log("Back clicked"),
  },
};

export const WithoutSlug: Story = {
  args: {
    title: "John Doe",
    createdAt: "2024-01-15T10:30:00Z",
    onBack: () => console.log("Back clicked"),
    onEdit: () => console.log("Edit clicked"),
  },
};

export const Minimal: Story = {
  args: {
    title: "Project Details",
    onBack: () => console.log("Back clicked"),
  },
};
