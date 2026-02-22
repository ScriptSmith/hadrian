import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import VectorStoreDetailPage from "./VectorStoreDetailPage";
import type {
  VectorStore,
  VectorStoreFile,
  VectorStoreFileListResponse,
} from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const createQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        staleTime: Infinity,
      },
    },
  });

const mockVectorStore: VectorStore = {
  id: "vs_abc123def456",
  name: "Product Documentation",
  description: "Embeddings for product documentation, user guides, and API references",
  status: "completed",
  embedding_model: "text-embedding-3-small",
  embedding_dimensions: 1536,
  usage_bytes: 4_250_000,
  file_counts: { total: 5, completed: 4, in_progress: 1, failed: 0, cancelled: 0 },
  owner_id: "org-123",
  owner_type: "organization",
  created_at: "2024-03-10T09:00:00Z",
  updated_at: "2024-06-15T14:30:00Z",
};

const mockFiles: VectorStoreFile[] = [
  {
    id: "file-001-getting-started",
    status: "completed",
    usage_bytes: 1_200_000,
    chunking_strategy: {
      type: "auto",
      auto: { max_chunk_size_tokens: 800, chunk_overlap_tokens: 400 },
    },
    created_at: "2024-03-10T09:05:00Z",
    updated_at: "2024-03-10T09:15:00Z",
  },
  {
    id: "file-002-api-reference",
    status: "completed",
    usage_bytes: 2_400_000,
    chunking_strategy: {
      type: "static",
      static: { max_chunk_size_tokens: 1600, chunk_overlap_tokens: 400 },
    },
    created_at: "2024-03-10T09:10:00Z",
    updated_at: "2024-03-10T09:25:00Z",
  },
  {
    id: "file-003-troubleshoot",
    status: "completed",
    usage_bytes: 450_000,
    chunking_strategy: {
      type: "auto",
      auto: { max_chunk_size_tokens: 800, chunk_overlap_tokens: 400 },
    },
    created_at: "2024-04-15T11:00:00Z",
    updated_at: "2024-04-15T11:10:00Z",
  },
  {
    id: "file-004-changelog",
    status: "in_progress",
    usage_bytes: 0,
    chunking_strategy: null,
    created_at: "2024-06-15T14:30:00Z",
    updated_at: "2024-06-15T14:30:00Z",
  },
  {
    id: "file-005-migration",
    status: "completed",
    usage_bytes: 200_000,
    chunking_strategy: {
      type: "auto",
      auto: { max_chunk_size_tokens: 800, chunk_overlap_tokens: 400 },
    },
    created_at: "2024-05-20T16:00:00Z",
    updated_at: "2024-05-20T16:08:00Z",
  },
];

const mockFilesResponse: VectorStoreFileListResponse = {
  data: mockFiles,
  has_more: false,
};

const emptyFilesResponse: VectorStoreFileListResponse = {
  data: [],
  has_more: false,
};

const vectorStoreId = "vs_abc123def456";

const createDecorator = () => (Story: React.ComponentType) => {
  const queryClient = createQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <ConfirmDialogProvider>
          <MemoryRouter initialEntries={[`/admin/vector-stores/${vectorStoreId}`]}>
            <Routes>
              <Route path="/admin/vector-stores/:vectorStoreId" element={<Story />} />
              <Route path="/admin/vector-stores" element={<div>Vector Stores List Page</div>} />
            </Routes>
          </MemoryRouter>
        </ConfirmDialogProvider>
      </ToastProvider>
    </QueryClientProvider>
  );
};

const meta: Meta<typeof VectorStoreDetailPage> = {
  title: "Admin/VectorStoreDetailPage",
  component: VectorStoreDetailPage,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [{ id: "heading-order", enabled: false }],
      },
    },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/v1/vector_stores/:vectorStoreId", () => {
          return HttpResponse.json(mockVectorStore);
        }),
        http.get("*/api/v1/vector_stores/:vectorStoreId/files", () => {
          return HttpResponse.json(mockFilesResponse);
        }),
        http.get("*/api/v1/vector_stores/:vectorStoreId/files/:fileId/chunks", () => {
          return HttpResponse.json({
            data: [
              {
                index: 0,
                text: "Getting started with the Hadrian AI Gateway is straightforward. First, download the binary for your platform...",
              },
              {
                index: 1,
                text: "Configure your providers in gateway.toml. Each provider needs a name, type, and API key reference...",
              },
            ],
            has_more: false,
          });
        }),
        http.post("*/api/v1/vector_stores/:vectorStoreId/search", () => {
          return HttpResponse.json({
            data: [
              {
                file_id: "file-001-getting-started",
                content: [{ type: "text", text: "Getting started with the Hadrian AI Gateway..." }],
                score: 0.92,
              },
              {
                file_id: "file-002-api-reference",
                content: [{ type: "text", text: "The API follows the OpenAI specification..." }],
                score: 0.85,
              },
            ],
          });
        }),
        http.delete("*/api/v1/vector_stores/:vectorStoreId/files/:fileId", () => {
          return HttpResponse.json({ id: "file-001", deleted: true });
        }),
        http.get("*/api/v1/files", () => {
          return HttpResponse.json({
            data: [
              {
                id: "file-existing-001",
                filename: "architecture-overview.md",
                bytes: 45_000,
                purpose: "assistants",
                created_at: "2024-05-01T10:00:00Z",
              },
            ],
            has_more: false,
          });
        }),
      ],
    },
  },
};

export const Loading: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/v1/vector_stores/:vectorStoreId", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockVectorStore);
        }),
        http.get("*/api/v1/vector_stores/:vectorStoreId/files", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockFilesResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/v1/vector_stores/:vectorStoreId", () => {
          return HttpResponse.json({
            ...mockVectorStore,
            usage_bytes: 0,
            file_counts: { total: 0, completed: 0, in_progress: 0, failed: 0, cancelled: 0 },
          });
        }),
        http.get("*/api/v1/vector_stores/:vectorStoreId/files", () => {
          return HttpResponse.json(emptyFilesResponse);
        }),
        http.post("*/api/v1/vector_stores/:vectorStoreId/search", () => {
          return HttpResponse.json({ data: [] });
        }),
        http.get("*/api/v1/files", () => {
          return HttpResponse.json({ data: [], has_more: false });
        }),
      ],
    },
  },
};

export const Error: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/v1/vector_stores/:vectorStoreId", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "Vector store not found" } },
            { status: 404 }
          );
        }),
      ],
    },
  },
};

export const WithFailedFiles: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/v1/vector_stores/:vectorStoreId", () => {
          return HttpResponse.json({
            ...mockVectorStore,
            file_counts: { total: 5, completed: 2, in_progress: 0, failed: 3, cancelled: 0 },
          });
        }),
        http.get("*/api/v1/vector_stores/:vectorStoreId/files", () => {
          const filesWithFailures: VectorStoreFile[] = [
            ...mockFiles.slice(0, 2),
            {
              id: "file-006-corrupted",
              status: "failed",
              usage_bytes: 0,
              chunking_strategy: null,
              last_error: {
                code: "server_error",
                message: "Text extraction failed: unsupported format",
              },
              created_at: "2024-06-14T10:00:00Z",
              updated_at: "2024-06-14T10:02:00Z",
            },
            {
              id: "file-007-too-large",
              status: "failed",
              usage_bytes: 0,
              chunking_strategy: null,
              last_error: { code: "server_error", message: "File exceeds maximum size limit" },
              created_at: "2024-06-14T11:00:00Z",
              updated_at: "2024-06-14T11:01:00Z",
            },
            {
              id: "file-008-timeout",
              status: "failed",
              usage_bytes: 0,
              chunking_strategy: null,
              last_error: {
                code: "server_error",
                message: "Processing timed out after 30 minutes",
              },
              created_at: "2024-06-14T12:00:00Z",
              updated_at: "2024-06-14T12:30:00Z",
            },
          ];
          return HttpResponse.json({
            data: filesWithFailures,
            has_more: false,
          });
        }),
        http.post("*/api/v1/vector_stores/:vectorStoreId/search", () => {
          return HttpResponse.json({ data: [] });
        }),
        http.get("*/api/v1/files", () => {
          return HttpResponse.json({ data: [], has_more: false });
        }),
      ],
    },
  },
};
