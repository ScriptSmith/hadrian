import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { AdminPromptFormModal } from "./PromptFormModal";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, staleTime: Infinity },
  },
});

const meta: Meta<typeof AdminPromptFormModal> = {
  title: "Admin/PromptFormModal",
  component: AdminPromptFormModal,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof AdminPromptFormModal>;

export const CreateMode: Story = {
  args: {
    open: true,
    onClose: () => console.log("Close"),
    ownerOverride: { type: "organization", id: "org_1" },
  },
};

export const EditMode: Story = {
  args: {
    open: true,
    onClose: () => console.log("Close"),
    ownerOverride: { type: "organization", id: "org_1" },
    editingPrompt: {
      id: "tpl_1",
      name: "Code Review Assistant",
      description: "Reviews code for best practices",
      content:
        "You are a code review assistant. Review the following {{language}} code:\n\n{{code}}",
      owner: { type: "organization", id: "org_1" },
      metadata: {
        variables: [
          {
            name: "language",
            label: "Language",
            type: "select",
            options: ["Python", "TypeScript", "Go"],
          },
          { name: "code", label: "Code", type: "textarea", required: true },
        ],
      },
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
  },
};
