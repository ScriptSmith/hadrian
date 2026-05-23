import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import ContainersPage from "./ContainersPage";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const now = Math.floor(Date.now() / 1000);

const mockContainers = {
  object: "list",
  data: [
    {
      id: "cntr_a1b2c3d4e5f6",
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
    },
    {
      id: "cntr_f6e5d4c3b2a1",
      object: "container",
      status: "expired",
      created_at: now - 86400,
      last_active_at: now - 84000,
      expires_at: now - 82800,
      idle_ttl_secs: 1200,
      runtime: "opensandbox",
    },
  ],
  has_more: false,
};

const meta: Meta<typeof ContainersPage> = {
  title: "Pages/ContainersPage",
  component: ContainersPage,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <MemoryRouter>
          <ToastProvider>
            <ConfirmDialogProvider>
              <Story />
            </ConfirmDialogProvider>
          </ToastProvider>
        </MemoryRouter>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    layout: "fullscreen",
    msw: {
      handlers: [http.get("*/v1/containers", () => HttpResponse.json(mockContainers))],
    },
  },
};

export default meta;
type Story = StoryObj<typeof ContainersPage>;

export const Default: Story = {};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/v1/containers", async () => {
          await new Promise((resolve) => setTimeout(resolve, 999999));
          return HttpResponse.json(mockContainers);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/v1/containers", () =>
          HttpResponse.json({ object: "list", data: [], has_more: false })
        ),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [http.get("*/v1/containers", () => HttpResponse.error())],
    },
  },
};
