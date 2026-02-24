import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import ApiKeyDetailPage from "./ApiKeyDetailPage";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const mockApiKey = {
  id: "key-my-1",
  name: "Production API Key",
  key_prefix: "gw_prod_abc",
  owner: { type: "user", user_id: "user-1" },
  budget_limit_cents: 10000,
  budget_period: "monthly",
  created_at: "2024-01-15T00:00:00Z",
  last_used_at: "2024-03-10T14:30:00Z",
  revoked_at: null,
  expires_at: "2025-12-31T23:59:59Z",
  rotation_grace_until: null,
  rotated_from_key_id: null,
  scopes: ["chat", "completions"],
  allowed_models: ["gpt-4o", "claude-*"],
  ip_allowlist: ["10.0.0.0/8", "192.168.1.0/24"],
  rate_limit_rpm: 1000,
  rate_limit_tpm: 100000,
};

const mockUsageSummary = {
  total_requests: 1250,
  total_input_tokens: 500000,
  total_output_tokens: 250000,
  total_cost_microcents: 125000000,
  by_model: [],
  by_user: [],
};

const commonHandlers = [
  http.get("*/admin/v1/me/api-keys/key-my-1", () => HttpResponse.json(mockApiKey)),
  http.get("*/admin/v1/me/usage/summary", () => HttpResponse.json(mockUsageSummary)),
  http.get("*/admin/v1/me/usage/timeseries", () => HttpResponse.json({ data: [] })),
  http.delete("*/admin/v1/me/api-keys/key-my-1", () => HttpResponse.json(null)),
  http.post("*/admin/v1/me/api-keys/key-my-1/rotate", () =>
    HttpResponse.json(
      {
        api_key: { ...mockApiKey, id: "key-my-2", key_prefix: "gw_new_xyz" },
        key: "gw_new_xyz_full_key_value",
      },
      { status: 201 }
    )
  ),
];

const meta: Meta<typeof ApiKeyDetailPage> = {
  title: "Pages/ApiKeyDetailPage",
  component: ApiKeyDetailPage,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [{ id: "heading-order", enabled: false }],
      },
    },
  },
  decorators: [
    (Story) => {
      const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false, staleTime: Infinity } },
      });
      return (
        <QueryClientProvider client={queryClient}>
          <ToastProvider>
            <ConfirmDialogProvider>
              <MemoryRouter initialEntries={["/api-keys/key-my-1"]}>
                <Routes>
                  <Route path="/api-keys/:keyId" element={<Story />} />
                </Routes>
              </MemoryRouter>
            </ConfirmDialogProvider>
          </ToastProvider>
        </QueryClientProvider>
      );
    },
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  parameters: {
    msw: {
      handlers: commonHandlers,
    },
  },
};

export const NoRestrictions: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/me/api-keys/key-my-1", () =>
          HttpResponse.json({
            ...mockApiKey,
            scopes: null,
            allowed_models: null,
            ip_allowlist: null,
            rate_limit_rpm: null,
            rate_limit_tpm: null,
          })
        ),
        ...commonHandlers.slice(1),
      ],
    },
  },
};

export const Revoked: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/me/api-keys/key-my-1", () =>
          HttpResponse.json({
            ...mockApiKey,
            revoked_at: "2024-03-01T00:00:00Z",
          })
        ),
        ...commonHandlers.slice(1),
      ],
    },
  },
};

export const Rotating: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/me/api-keys/key-my-1", () =>
          HttpResponse.json({
            ...mockApiKey,
            rotation_grace_until: new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString(),
          })
        ),
        ...commonHandlers.slice(1),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/me/api-keys/key-my-1", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockApiKey);
        }),
        ...commonHandlers.slice(1),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/me/api-keys/key-my-1", () =>
          HttpResponse.json({ error: "Not found" }, { status: 404 })
        ),
        ...commonHandlers.slice(1),
      ],
    },
  },
};
