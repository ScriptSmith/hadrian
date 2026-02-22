import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Button } from "@/components/Button/Button";

import { ApiKeyFormModal } from "./ApiKeyFormModal";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const meta: Meta<typeof ApiKeyFormModal> = {
  title: "Admin/ApiKeyFormModal",
  component: ApiKeyFormModal,
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
type Story = StoryObj<typeof ApiKeyFormModal>;

const mockOrganizations = [
  {
    id: "org_1",
    slug: "acme-corp",
    name: "Acme Corporation",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "org_2",
    slug: "startup-inc",
    name: "Startup Inc",
    created_at: "2024-01-02T00:00:00Z",
    updated_at: "2024-01-02T00:00:00Z",
  },
  {
    id: "org_3",
    slug: "enterprise-llc",
    name: "Enterprise LLC",
    created_at: "2024-01-03T00:00:00Z",
    updated_at: "2024-01-03T00:00:00Z",
  },
];

export const Default: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onSubmit: (data) => console.log("Submit", data),
    organizations: mockOrganizations,
  },
};

export const Loading: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onSubmit: (data) => console.log("Submit", data),
    organizations: mockOrganizations,
    isLoading: true,
  },
};

export const NoOrganizations: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onSubmit: (data) => console.log("Submit", data),
    organizations: [],
  },
};

export const SingleOrganization: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onSubmit: (data) => console.log("Submit", data),
    organizations: [mockOrganizations[0]],
  },
};

// Interactive story that can open/close the modal and demonstrate the form
function InteractiveDemo() {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="p-4">
      <Button onClick={() => setIsOpen(true)}>Open API Key Form</Button>
      <ApiKeyFormModal
        isOpen={isOpen}
        onClose={() => setIsOpen(false)}
        onSubmit={(data) => {
          console.log("Submitted:", data);
          setIsOpen(false);
        }}
        organizations={mockOrganizations}
      />
    </div>
  );
}

export const Interactive: Story = {
  render: () => <InteractiveDemo />,
};
