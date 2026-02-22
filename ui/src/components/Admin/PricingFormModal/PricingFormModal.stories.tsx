import type { Meta, StoryObj } from "@storybook/react";

import { PricingFormModal } from "./PricingFormModal";

const meta: Meta<typeof PricingFormModal> = {
  title: "Admin/PricingFormModal",
  component: PricingFormModal,
  parameters: {
    layout: "centered",
  },
};

export default meta;
type Story = StoryObj<typeof PricingFormModal>;

const mockPricing = {
  id: "pricing_1",
  provider: "openai",
  model: "gpt-4",
  input_per_1m_tokens: 30000000, // $30 in microcents
  output_per_1m_tokens: 60000000, // $60 in microcents
  cached_input_per_1m_tokens: 15000000, // $15 in microcents
  reasoning_per_1m_tokens: null,
  per_request: null,
  per_image: null,
  source: "manual" as const,
  owner: { type: "global" as const },
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-01T00:00:00Z",
};

export const CreateMode: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
  },
};

export const EditMode: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
    editingPricing: mockPricing,
  },
};

export const EditModeWithAllFields: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
    editingPricing: {
      ...mockPricing,
      reasoning_per_1m_tokens: 150000000, // $150 in microcents
      per_request: 1000, // $0.001 in microcents
      per_image: 5000000, // $5 in microcents
    },
  },
};

export const Loading: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
    isLoading: true,
  },
};

export const EditModeProviderAPI: Story = {
  args: {
    isOpen: true,
    onClose: () => console.log("Close"),
    onCreateSubmit: (data) => console.log("Create", data),
    onEditSubmit: (data) => console.log("Edit", data),
    editingPricing: {
      ...mockPricing,
      source: "provider_api" as const,
    },
  },
};
