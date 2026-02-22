import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import KnowledgeBasesPage from "./KnowledgeBasesPage";
import { ToastProvider } from "@/components/Toast/Toast";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
    },
  },
});

const mockOrganizations = {
  data: [
    {
      id: "org-1",
      name: "Acme Corp",
      slug: "acme-corp",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
  ],
  total: 1,
};

const mockVectorStores = {
  data: [
    {
      id: "vs_abc123",
      name: "Product Documentation",
      description: "All product manuals and API documentation",
      status: "completed",
      file_counts: { total: 45, completed: 45, in_progress: 0, failed: 0, cancelled: 0 },
      usage_bytes: 15728640,
      embedding_model: "text-embedding-3-small",
      embedding_dimensions: 1536,
      owner_id: "org-1",
      owner_type: "organization",
      created_at: "2024-01-15T00:00:00Z",
      updated_at: "2024-02-01T00:00:00Z",
    },
    {
      id: "vs_def456",
      name: "Support Articles",
      description: "Customer support knowledge base",
      status: "completed",
      file_counts: { total: 128, completed: 128, in_progress: 0, failed: 0, cancelled: 0 },
      usage_bytes: 52428800,
      embedding_model: "text-embedding-3-small",
      embedding_dimensions: 1536,
      owner_id: "org-1",
      owner_type: "organization",
      created_at: "2024-02-01T00:00:00Z",
      updated_at: "2024-02-15T00:00:00Z",
    },
    {
      id: "vs_ghi789",
      name: "Training Data",
      description: null,
      status: "in_progress",
      file_counts: { total: 20, completed: 12, in_progress: 8, failed: 0, cancelled: 0 },
      usage_bytes: 8388608,
      embedding_model: "text-embedding-3-large",
      embedding_dimensions: 3072,
      owner_id: "org-1",
      owner_type: "organization",
      created_at: "2024-03-01T00:00:00Z",
      updated_at: "2024-03-10T00:00:00Z",
    },
  ],
  first_id: "vs_abc123",
  last_id: "vs_ghi789",
  has_more: false,
};

const defaultHandlers = [
  http.get("*/api/admin/v1/organizations", () => {
    return HttpResponse.json(mockOrganizations);
  }),
  http.get("*/v1/vector_stores", () => {
    return HttpResponse.json(mockVectorStores);
  }),
];

const meta: Meta<typeof KnowledgeBasesPage> = {
  title: "Pages/KnowledgeBasesPage",
  component: KnowledgeBasesPage,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <ToastProvider>
            <Story />
          </ToastProvider>
        </MemoryRouter>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [{ id: "heading-order", enabled: false }],
      },
    },
    msw: {
      handlers: defaultHandlers,
    },
  },
};

export default meta;
type Story = StoryObj<typeof KnowledgeBasesPage>;

export const Default: Story = {};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", async () => {
          await new Promise((resolve) => setTimeout(resolve, 999999));
          return HttpResponse.json(mockOrganizations);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/v1/vector_stores", () => {
          return HttpResponse.json({ data: [], first_id: null, last_id: null, has_more: false });
        }),
      ],
    },
  },
};

export const NoOrganizations: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json({ data: [], total: 0 });
        }),
      ],
    },
  },
};

export const WithProcessingFiles: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/v1/vector_stores", () => {
          return HttpResponse.json({
            data: [
              {
                id: "vs_processing",
                name: "New Knowledge Base",
                description: "Currently processing uploaded files",
                status: "in_progress",
                file_counts: { total: 50, completed: 15, in_progress: 35, failed: 0, cancelled: 0 },
                usage_bytes: 5242880,
                embedding_model: "text-embedding-3-small",
                embedding_dimensions: 1536,
                owner_id: "org-1",
                owner_type: "organization",
                created_at: "2024-03-15T00:00:00Z",
                updated_at: "2024-03-15T10:30:00Z",
              },
            ],
            first_id: "vs_processing",
            last_id: "vs_processing",
            has_more: false,
          });
        }),
      ],
    },
  },
};

export const ManyKnowledgeBases: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrganizations);
        }),
        http.get("*/v1/vector_stores", () => {
          return HttpResponse.json({
            data: Array.from({ length: 9 }, (_, i) => ({
              id: `vs_${i + 1}`,
              name: `Knowledge Base ${i + 1}`,
              description: i % 2 === 0 ? `Description for KB ${i + 1}` : null,
              status: i === 2 ? "in_progress" : "completed",
              file_counts: {
                total: 10 + i * 5,
                completed: i === 2 ? 5 : 10 + i * 5,
                in_progress: i === 2 ? 5 + i * 5 : 0,
                failed: 0,
                cancelled: 0,
              },
              usage_bytes: (i + 1) * 1048576,
              embedding_model: "text-embedding-3-small",
              embedding_dimensions: 1536,
              owner_id: "org-1",
              owner_type: "organization",
              created_at: new Date(2024, 0, i + 1).toISOString(),
              updated_at: new Date(2024, 0, i + 15).toISOString(),
            })),
            first_id: "vs_1",
            last_id: "vs_9",
            has_more: false,
          });
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/organizations", () => {
          return HttpResponse.error();
        }),
      ],
    },
  },
};
