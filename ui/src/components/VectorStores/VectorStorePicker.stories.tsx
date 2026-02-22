import type { Meta, StoryObj } from "@storybook/react";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";

import type { VectorStore } from "@/api/generated/types.gen";
import { VectorStorePicker } from "./VectorStorePicker";

// Mock vector store data
const mockVectorStores: VectorStore[] = [
  {
    id: "vs_001",
    name: "Product Documentation",
    description: "Technical documentation for all products",
    object: "vector_store",
    status: "completed",
    owner_type: "organization",
    owner_id: "org_123",
    embedding_model: "text-embedding-3-small",
    embedding_dimensions: 1536,
    usage_bytes: 2500000,
    file_counts: {
      in_progress: 0,
      completed: 15,
      failed: 0,
      cancelled: 0,
      total: 15,
    },
    created_at: "2024-12-01T10:00:00Z",
    updated_at: "2024-12-14T15:30:00Z",
  },
  {
    id: "vs_002",
    name: "Customer Support KB",
    description: "Knowledge base for customer support team",
    object: "vector_store",
    status: "completed",
    owner_type: "organization",
    owner_id: "org_123",
    embedding_model: "text-embedding-3-small",
    embedding_dimensions: 1536,
    usage_bytes: 5000000,
    file_counts: {
      in_progress: 0,
      completed: 42,
      failed: 2,
      cancelled: 0,
      total: 44,
    },
    created_at: "2024-11-15T08:00:00Z",
    updated_at: "2024-12-10T12:00:00Z",
  },
  {
    id: "vs_003",
    name: "Legal Documents",
    description: "Contracts and legal documentation",
    object: "vector_store",
    status: "in_progress",
    owner_type: "organization",
    owner_id: "org_123",
    embedding_model: "text-embedding-ada-002",
    embedding_dimensions: 1536,
    usage_bytes: 1000000,
    file_counts: {
      in_progress: 5,
      completed: 8,
      failed: 0,
      cancelled: 0,
      total: 13,
    },
    created_at: "2024-12-14T09:00:00Z",
    updated_at: "2024-12-14T09:30:00Z",
  },
  {
    id: "vs_004",
    name: "Research Papers",
    description: null,
    object: "vector_store",
    status: "completed",
    owner_type: "organization",
    owner_id: "org_123",
    embedding_model: "text-embedding-3-large",
    embedding_dimensions: 3072,
    usage_bytes: 15000000,
    file_counts: {
      in_progress: 0,
      completed: 128,
      failed: 0,
      cancelled: 0,
      total: 128,
    },
    created_at: "2024-10-01T00:00:00Z",
    updated_at: "2024-12-01T00:00:00Z",
  },
  {
    id: "vs_005",
    name: "Archived Data",
    object: "vector_store",
    status: "expired",
    owner_type: "organization",
    owner_id: "org_123",
    embedding_model: "text-embedding-ada-002",
    embedding_dimensions: 1536,
    usage_bytes: 500000,
    file_counts: {
      in_progress: 0,
      completed: 5,
      failed: 0,
      cancelled: 0,
      total: 5,
    },
    created_at: "2024-06-01T00:00:00Z",
    updated_at: "2024-06-15T00:00:00Z",
    expires_at: "2024-09-01T00:00:00Z",
  },
];

// Interactive wrapper to handle state
function InteractiveWrapper({
  initialSelected = [],
  ...props
}: {
  initialSelected?: string[];
  availableStores: VectorStore[];
  maxStores?: number;
  isLoading?: boolean;
}) {
  const [open, setOpen] = useState(true);
  const [selectedIds, setSelectedIds] = useState<string[]>(initialSelected);

  return (
    <div className="p-4">
      <div className="mb-4">
        <p className="text-sm text-muted-foreground mb-2">
          Selected IDs: {selectedIds.length > 0 ? selectedIds.join(", ") : "(none)"}
        </p>
        <button
          onClick={() => setOpen(true)}
          className="px-3 py-1.5 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90"
        >
          Open Picker
        </button>
      </div>
      <VectorStorePicker
        open={open}
        onClose={() => setOpen(false)}
        selectedIds={selectedIds}
        onIdsChange={setSelectedIds}
        {...props}
      />
    </div>
  );
}

const meta: Meta<typeof VectorStorePicker> = {
  title: "VectorStores/VectorStorePicker",
  component: VectorStorePicker,
  parameters: {
    layout: "fullscreen",
  },
};

export default meta;
type Story = StoryObj<typeof VectorStorePicker>;

/** Default picker with available stores */
export const Default: Story = {
  render: () => <InteractiveWrapper availableStores={mockVectorStores} />,
  play: async () => {
    // Component uses portal, so we need to query document.body
    const body = within(document.body);

    // Verify the picker is open with the title
    await expect(body.getByText("Select Knowledge Bases")).toBeInTheDocument();

    // Verify search input is present
    const searchInput = body.getByPlaceholderText("Search knowledge bases...");
    await expect(searchInput).toBeInTheDocument();

    // Verify stores are listed
    await expect(body.getByText("Product Documentation")).toBeInTheDocument();
    await expect(body.getByText("Customer Support KB")).toBeInTheDocument();

    // Verify initial selection count
    await expect(body.getByText("0/10")).toBeInTheDocument();

    // Click on a store to select it
    const productDocButton = body.getByText("Product Documentation").closest("button");
    await userEvent.click(productDocButton!);

    // Verify selection count updates (should show 1/10)
    await expect(body.getByText("1/10")).toBeInTheDocument();

    // Verify "Selected" section appears
    await expect(body.getByText("Selected")).toBeInTheDocument();

    // Re-query the button since it moved to "Selected" section
    const productDocButtonInSelected = body.getByText("Product Documentation").closest("button");
    await userEvent.click(productDocButtonInSelected!);

    // Verify selection count is back to 0/10
    await expect(body.getByText("0/10")).toBeInTheDocument();
  },
};

/** Picker with some stores already selected */
export const WithSelectedStores: Story = {
  render: () => (
    <InteractiveWrapper availableStores={mockVectorStores} initialSelected={["vs_001", "vs_002"]} />
  ),
  play: async () => {
    const body = within(document.body);

    // Verify initial selection count
    await expect(body.getByText("2/10")).toBeInTheDocument();

    // Verify "Selected" section shows count
    await expect(body.getByText("Selected")).toBeInTheDocument();
    // The count "2" appears next to "Selected"
    const selectedSection = body.getByText("Selected").closest("div");
    await expect(selectedSection).toHaveTextContent("2");

    // Verify "Clear all" button appears when items are selected
    await expect(body.getByText("Clear all")).toBeInTheDocument();

    // Click "Clear all"
    await userEvent.click(body.getByText("Clear all"));

    // Verify selection is cleared
    await expect(body.getByText("0/10")).toBeInTheDocument();
  },
};

/** Picker at max capacity */
export const MaxCapacity: Story = {
  render: () => (
    <InteractiveWrapper
      availableStores={mockVectorStores}
      initialSelected={["vs_001", "vs_002", "vs_003"]}
      maxStores={3}
    />
  ),
  play: async () => {
    const body = within(document.body);

    // Verify at max capacity (3/3)
    await expect(body.getByText("3/3")).toBeInTheDocument();

    // Verify unselected stores are disabled (have opacity)
    const researchPapersButton = body.getByText("Research Papers").closest("button");
    await expect(researchPapersButton).toBeDisabled();

    // Deselect one to make room
    const productDocButton = body.getByText("Product Documentation").closest("button");
    await userEvent.click(productDocButton!);

    // Verify count is now 2/3
    await expect(body.getByText("2/3")).toBeInTheDocument();

    // Now Research Papers should be enabled
    const researchPapersButtonAfter = body.getByText("Research Papers").closest("button");
    await expect(researchPapersButtonAfter).toBeEnabled();
  },
};

/** Empty state - no stores available */
export const NoStoresAvailable: Story = {
  render: () => <InteractiveWrapper availableStores={[]} />,
  play: async () => {
    const body = within(document.body);

    // Verify empty state message
    await expect(body.getByText("No knowledge bases available")).toBeInTheDocument();

    // Verify selection count shows 0/10
    await expect(body.getByText("0/10")).toBeInTheDocument();
  },
};

/** Loading state */
export const Loading: Story = {
  render: () => <InteractiveWrapper availableStores={[]} isLoading={true} />,
  play: async () => {
    const body = within(document.body);

    // Verify loading spinner is present (Spinner component renders an SVG with animate-spin)
    const spinner = document.body.querySelector(".animate-spin");
    await expect(spinner).toBeInTheDocument();

    // Verify no "No knowledge bases" message while loading
    await expect(body.queryByText("No knowledge bases available")).not.toBeInTheDocument();
  },
};

/** Single store available */
export const SingleStore: Story = {
  render: () => <InteractiveWrapper availableStores={[mockVectorStores[0]]} />,
  play: async () => {
    const body = within(document.body);

    // Verify single store is shown
    await expect(body.getByText("Product Documentation")).toBeInTheDocument();

    // Test search filtering
    const searchInput = body.getByPlaceholderText("Search knowledge bases...");
    await userEvent.type(searchInput, "nonexistent");

    // Verify "No knowledge bases found" message appears
    await expect(body.getByText("No knowledge bases found")).toBeInTheDocument();

    // Clear search
    await userEvent.clear(searchInput);

    // Store should reappear
    await expect(body.getByText("Product Documentation")).toBeInTheDocument();
  },
};

/** Low max stores limit */
export const LowMaxLimit: Story = {
  render: () => <InteractiveWrapper availableStores={mockVectorStores} maxStores={2} />,
  play: async () => {
    const body = within(document.body);

    // Verify max stores limit is shown as 0/2
    await expect(body.getByText("0/2")).toBeInTheDocument();

    // Select two stores
    const productDocButton = body.getByText("Product Documentation").closest("button");
    await userEvent.click(productDocButton!);
    await expect(body.getByText("1/2")).toBeInTheDocument();

    const customerSupportButton = body.getByText("Customer Support KB").closest("button");
    await userEvent.click(customerSupportButton!);
    await expect(body.getByText("2/2")).toBeInTheDocument();

    // Third store should be disabled now
    const legalDocsButton = body.getByText("Legal Documents").closest("button");
    await expect(legalDocsButton).toBeDisabled();
  },
};
