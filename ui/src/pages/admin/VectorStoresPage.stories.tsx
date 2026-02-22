import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import VectorStoresPage from "./VectorStoresPage";
import type {
  VectorStore,
  VectorStoreListResponse,
  Organization,
  OrganizationListResponse,
} from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const mockOrgs: Organization[] = [
  {
    id: "org-123",
    slug: "acme-corp",
    name: "Acme Corporation",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "org-456",
    slug: "stark-industries",
    name: "Stark Industries",
    created_at: "2024-02-01T00:00:00Z",
    updated_at: "2024-02-01T00:00:00Z",
  },
];

const mockOrgsResponse: OrganizationListResponse = {
  data: mockOrgs,
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const mockVectorStores: VectorStore[] = [
  {
    id: "vs_abc123",
    name: "Product Documentation",
    description: "Embeddings for product documentation and user guides",
    status: "completed",
    embedding_model: "text-embedding-3-small",
    embedding_dimensions: 1536,
    usage_bytes: 4_250_000,
    file_counts: { total: 24, completed: 22, in_progress: 2, failed: 0, cancelled: 0 },
    owner_id: "org-123",
    owner_type: "organization",
    created_at: "2024-03-10T09:00:00Z",
    updated_at: "2024-06-15T14:30:00Z",
  },
  {
    id: "vs_def456",
    name: "Engineering Wiki",
    description: "Internal engineering knowledge base",
    status: "completed",
    embedding_model: "text-embedding-3-large",
    embedding_dimensions: 3072,
    usage_bytes: 12_800_000,
    file_counts: { total: 156, completed: 156, in_progress: 0, failed: 0, cancelled: 0 },
    owner_id: "org-123",
    owner_type: "organization",
    created_at: "2024-04-05T11:20:00Z",
    updated_at: "2024-06-10T08:45:00Z",
  },
  {
    id: "vs_ghi789",
    name: "Support Tickets Archive",
    description: null,
    status: "in_progress",
    embedding_model: "text-embedding-3-small",
    embedding_dimensions: 1536,
    usage_bytes: 890_000,
    file_counts: { total: 48, completed: 30, in_progress: 15, failed: 3, cancelled: 0 },
    owner_id: "org-123",
    owner_type: "organization",
    created_at: "2024-05-20T16:00:00Z",
    updated_at: "2024-06-15T16:00:00Z",
  },
  {
    id: "vs_jkl012",
    name: "Legal Contracts",
    description: "Contract templates and legal documents for RAG",
    status: "expired",
    embedding_model: "text-embedding-ada-002",
    embedding_dimensions: 1536,
    usage_bytes: 2_100_000,
    file_counts: { total: 12, completed: 12, in_progress: 0, failed: 0, cancelled: 0 },
    owner_id: "org-123",
    owner_type: "organization",
    created_at: "2024-01-15T13:00:00Z",
    updated_at: "2024-03-01T09:00:00Z",
  },
];

const mockVectorStoresResponse: VectorStoreListResponse = {
  data: mockVectorStores,
  has_more: false,
};

const emptyVectorStoresResponse: VectorStoreListResponse = {
  data: [],
  has_more: false,
};

const meta: Meta<typeof VectorStoresPage> = {
  title: "Admin/VectorStoresPage",
  component: VectorStoresPage,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [{ id: "heading-order", enabled: false }],
      },
    },
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <ConfirmDialogProvider>
            <MemoryRouter initialEntries={["/admin/vector-stores"]}>
              <Routes>
                <Route path="/admin/vector-stores" element={<Story />} />
              </Routes>
            </MemoryRouter>
          </ConfirmDialogProvider>
        </ToastProvider>
      </QueryClientProvider>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/api/v1/vector_stores", () => {
          return HttpResponse.json(mockVectorStoresResponse);
        }),
        http.post("*/api/v1/vector_stores", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newStore: VectorStore = {
            id: `vs_${Date.now()}`,
            name: body.name as string,
            description: (body.description as string) || null,
            status: "in_progress",
            embedding_model: "text-embedding-3-small",
            embedding_dimensions: 1536,
            usage_bytes: 0,
            file_counts: { total: 0, completed: 0, in_progress: 0, failed: 0, cancelled: 0 },
            owner_id: "org-123",
            owner_type: "organization",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newStore, { status: 201 });
        }),
        http.post("*/api/v1/vector_stores/:vectorStoreId", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const updated: VectorStore = {
            ...mockVectorStores[0],
            ...body,
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(updated);
        }),
        http.delete("*/api/v1/vector_stores/:vectorStoreId", () => {
          return HttpResponse.json({ id: "vs_abc123", deleted: true });
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/api/v1/vector_stores", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockVectorStoresResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/api/v1/vector_stores", () => {
          return HttpResponse.json(emptyVectorStoresResponse);
        }),
        http.post("*/api/v1/vector_stores", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newStore: VectorStore = {
            id: `vs_${Date.now()}`,
            name: body.name as string,
            description: (body.description as string) || null,
            status: "in_progress",
            embedding_model: "text-embedding-3-small",
            embedding_dimensions: 1536,
            usage_bytes: 0,
            file_counts: { total: 0, completed: 0, in_progress: 0, failed: 0, cancelled: 0 },
            owner_id: "org-123",
            owner_type: "organization",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newStore, { status: 201 });
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/api/v1/vector_stores", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
      ],
    },
  },
};

export const NoOrganizations: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json({
            data: [],
            pagination: {
              limit: 100,
              has_more: false,
            },
          });
        }),
      ],
    },
  },
};
