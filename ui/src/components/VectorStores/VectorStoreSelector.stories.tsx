import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse, delay } from "msw";
import { useState } from "react";

import type { VectorStore } from "@/api/generated/types.gen";
import { VectorStoreSelector } from "./VectorStoreSelector";

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
];

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

// Interactive wrapper to handle state
function InteractiveWrapper({
  initialSelected = [],
  disabled = false,
  compact = false,
  maxStores,
}: {
  initialSelected?: string[];
  disabled?: boolean;
  compact?: boolean;
  maxStores?: number;
}) {
  const [selectedIds, setSelectedIds] = useState<string[]>(initialSelected);

  return (
    <div className="p-4">
      <div className="mb-4">
        <p className="text-sm text-muted-foreground">
          Selected IDs: {selectedIds.length > 0 ? selectedIds.join(", ") : "(none)"}
        </p>
      </div>
      <VectorStoreSelector
        selectedIds={selectedIds}
        onIdsChange={setSelectedIds}
        ownerType="organization"
        ownerId="org_123"
        disabled={disabled}
        compact={compact}
        maxStores={maxStores}
      />
    </div>
  );
}

const meta: Meta<typeof VectorStoreSelector> = {
  title: "VectorStores/VectorStoreSelector",
  component: VectorStoreSelector,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <Story />
      </QueryClientProvider>
    ),
  ],
  parameters: {
    layout: "padded",
    msw: {
      handlers: [
        http.get("*/api/v1/vector_stores", async () => {
          await delay(300);
          return HttpResponse.json({
            object: "list",
            data: mockVectorStores,
            first_id: mockVectorStores[0]?.id,
            last_id: mockVectorStores[mockVectorStores.length - 1]?.id,
            has_more: false,
          });
        }),
      ],
    },
  },
};

export default meta;
type Story = StoryObj<typeof VectorStoreSelector>;

/** Default selector - no stores selected */
export const Default: Story = {
  render: () => <InteractiveWrapper />,
};

/** Selector with stores already selected */
export const WithSelectedStores: Story = {
  render: () => <InteractiveWrapper initialSelected={["vs_001", "vs_002"]} />,
};

/** Selector at max capacity */
export const MaxCapacity: Story = {
  render: () => (
    <InteractiveWrapper initialSelected={["vs_001", "vs_002", "vs_003"]} maxStores={3} />
  ),
};

/** Disabled selector */
export const Disabled: Story = {
  render: () => <InteractiveWrapper initialSelected={["vs_001"]} disabled={true} />,
};

/** Compact mode - minimal UI when empty */
export const Compact: Story = {
  render: () => <InteractiveWrapper compact={true} />,
};

/** Compact mode with selections */
export const CompactWithSelections: Story = {
  render: () => <InteractiveWrapper compact={true} initialSelected={["vs_001"]} />,
};

/** Low max stores limit */
export const LowMaxLimit: Story = {
  render: () => <InteractiveWrapper maxStores={2} />,
};

/** Single store selected */
export const SingleStoreSelected: Story = {
  render: () => <InteractiveWrapper initialSelected={["vs_001"]} />,
};
