import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import ContainerDetailPage from "./ContainerDetailPage";
import { AuthProvider } from "@/auth";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const now = Math.floor(Date.now() / 1000);
const CONTAINER_ID = "cntr_a1b2c3d4e5f6";

const mockContainer = {
  id: CONTAINER_ID,
  object: "container",
  status: "active",
  created_at: now - 3600,
  last_active_at: now - 120,
  expires_at: now + 1080,
  idle_ttl_secs: 1200,
  runtime: "microsandbox",
  name: "data-analysis",
  memory_limit: "512m",
  memory_limit_mb: 512,
};

const mockFiles = {
  object: "list",
  data: [
    {
      id: "cfile_111",
      object: "container.file",
      container_id: CONTAINER_ID,
      path: "/mnt/data/residuals.csv",
      filename: "residuals.csv",
      bytes: 20480,
      source: "user",
      content_type: "text/csv",
      created_at: now - 3000,
    },
    {
      id: "cfile_222",
      object: "container.file",
      container_id: CONTAINER_ID,
      path: "/mnt/data/chart.png",
      filename: "chart.png",
      bytes: 153600,
      source: "assistant",
      content_type: "image/png",
      created_at: now - 600,
    },
  ],
  has_more: false,
};

const meta: Meta<typeof ContainerDetailPage> = {
  title: "Pages/ContainerDetailPage",
  component: ContainerDetailPage,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <AuthProvider>
          <MemoryRouter initialEntries={[`/containers/${CONTAINER_ID}`]}>
            <ToastProvider>
              <ConfirmDialogProvider>
                <Routes>
                  <Route path="/containers/:containerId" element={<Story />} />
                </Routes>
              </ConfirmDialogProvider>
            </ToastProvider>
          </MemoryRouter>
        </AuthProvider>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    layout: "fullscreen",
    msw: {
      handlers: [
        http.get(`*/v1/containers/${CONTAINER_ID}/files`, () => HttpResponse.json(mockFiles)),
        http.get(`*/v1/containers/${CONTAINER_ID}`, () => HttpResponse.json(mockContainer)),
      ],
    },
  },
};

export default meta;
type Story = StoryObj<typeof ContainerDetailPage>;

export const Default: Story = {};

export const NoFiles: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get(`*/v1/containers/${CONTAINER_ID}/files`, () =>
          HttpResponse.json({ object: "list", data: [], has_more: false })
        ),
        http.get(`*/v1/containers/${CONTAINER_ID}`, () => HttpResponse.json(mockContainer)),
      ],
    },
  },
};

export const NotFound: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get(`*/v1/containers/${CONTAINER_ID}/files`, () =>
          HttpResponse.json({ object: "list", data: [], has_more: false })
        ),
        http.get(`*/v1/containers/${CONTAINER_ID}`, () => HttpResponse.error()),
      ],
    },
  },
};
