import type { Meta, StoryObj } from "@storybook/react";

import { AddMemberModal } from "./AddMemberModal";

const meta: Meta<typeof AddMemberModal> = {
  title: "Admin/AddMemberModal",
  component: AddMemberModal,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof AddMemberModal>;

const mockUsers = [
  {
    id: "1",
    external_id: "user_1",
    name: "John Doe",
    email: "john@example.com",
    created_at: "2024-01-01",
    updated_at: "2024-01-01",
  },
  {
    id: "2",
    external_id: "user_2",
    name: "Jane Smith",
    email: "jane@example.com",
    created_at: "2024-01-02",
    updated_at: "2024-01-02",
  },
  {
    id: "3",
    external_id: "user_3",
    name: null,
    email: "bob@example.com",
    created_at: "2024-01-03",
    updated_at: "2024-01-03",
  },
];

export const Default: Story = {
  args: {
    open: true,
    onClose: () => console.log("Close"),
    onSubmit: (userId) => console.log("Submit", userId),
    availableUsers: mockUsers,
  },
};

export const Loading: Story = {
  args: {
    open: true,
    onClose: () => console.log("Close"),
    onSubmit: (userId) => console.log("Submit", userId),
    availableUsers: mockUsers,
    isLoading: true,
  },
};

export const NoAvailableUsers: Story = {
  args: {
    open: true,
    onClose: () => console.log("Close"),
    onSubmit: (userId) => console.log("Submit", userId),
    availableUsers: [],
    emptyMessage: "All users are already members of this organization.",
  },
};

export const CustomTitle: Story = {
  args: {
    open: true,
    onClose: () => console.log("Close"),
    onSubmit: (userId) => console.log("Submit", userId),
    availableUsers: mockUsers,
    title: "Add Team Member",
  },
};
