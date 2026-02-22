import type { Meta, StoryObj } from "@storybook/react";

import { VectorStoreFormModal } from "./VectorStoreFormModal";

const meta: Meta<typeof VectorStoreFormModal> = {
  title: "Admin/VectorStoreFormModal",
  component: VectorStoreFormModal,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof VectorStoreFormModal>;

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
];

const mockVectorStore = {
  id: "vs_1",
  name: "knowledge-base",
  description: "Company documentation and FAQs",
  owner_type: "organization" as const,
  owner_id: "org_1",
  status: "completed" as const,
  embedding_model: "text-embedding-3-small",
  embedding_dimensions: 1536,
  usage_bytes: 1024000,
  file_counts: {
    cancelled: 0,
    completed: 10,
    failed: 0,
    in_progress: 2,
    total: 12,
  },
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-15T00:00:00Z",
};

export const CreateMode: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
    organizations: mockOrganizations,
  },
};

export const EditMode: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
    editingStore: mockVectorStore,
    organizations: mockOrganizations,
  },
};

export const Loading: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
    organizations: mockOrganizations,
    isLoading: true,
  },
};

export const NoOrganizations: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
    organizations: [],
  },
};

export const EditModeWithLargeEmbeddings: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
    editingStore: {
      ...mockVectorStore,
      embedding_model: "text-embedding-3-large",
      embedding_dimensions: 3072,
    },
    organizations: mockOrganizations,
  },
};
