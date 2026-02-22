import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import ProjectDetailPage from "./ProjectDetailPage";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const mockProject = {
  id: "proj-1",
  name: "Production API",
  slug: "production-api",
  org_id: "org-1",
  team_id: null,
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-01-01T00:00:00Z",
};

const mockMembers = {
  data: [
    {
      id: "user-1",
      name: "Alice Smith",
      email: "alice@acme.com",
      external_id: "alice-123",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
    {
      id: "user-2",
      name: "Bob Johnson",
      email: "bob@acme.com",
      external_id: "bob-456",
      created_at: "2024-02-01T00:00:00Z",
      updated_at: "2024-02-01T00:00:00Z",
    },
  ],
  pagination: { limit: 100, has_more: false },
};

const mockApiKeys = {
  data: [
    {
      id: "key-1",
      name: "Production Key",
      key_prefix: "hd-prod",
      revoked_at: null,
      expires_at: null,
      created_at: "2024-06-01T00:00:00Z",
    },
    {
      id: "key-2",
      name: "Staging Key",
      key_prefix: "hd-stag",
      revoked_at: "2024-07-01T00:00:00Z",
      expires_at: null,
      created_at: "2024-05-01T00:00:00Z",
    },
  ],
  pagination: { limit: 100, has_more: false },
};

const mockProviders = {
  data: [
    {
      id: "prov-1",
      name: "project-openai",
      provider_type: "open_ai",
      base_url: "https://api.openai.com/v1",
      has_api_key: true,
      config: null,
      models: ["gpt-4o", "gpt-4o-mini"],
      owner: { type: "project", project_id: "proj-1" },
      is_enabled: true,
      created_at: "2024-06-01T00:00:00Z",
      updated_at: "2024-06-01T00:00:00Z",
    },
    {
      id: "prov-2",
      name: "project-anthropic",
      provider_type: "anthropic",
      base_url: "https://api.anthropic.com",
      has_api_key: true,
      config: null,
      models: ["claude-sonnet-4-20250514"],
      owner: { type: "project", project_id: "proj-1" },
      is_enabled: true,
      created_at: "2024-06-02T00:00:00Z",
      updated_at: "2024-06-02T00:00:00Z",
    },
    {
      id: "prov-3",
      name: "disabled-provider",
      provider_type: "open_ai",
      base_url: "https://custom.api.com/v1",
      has_api_key: false,
      config: null,
      models: [],
      owner: { type: "project", project_id: "proj-1" },
      is_enabled: false,
      created_at: "2024-06-03T00:00:00Z",
      updated_at: "2024-06-03T00:00:00Z",
    },
  ],
  pagination: { limit: 100, has_more: false },
};

const mockPricing = {
  data: [
    {
      id: "price-1",
      model: "gpt-4o",
      provider: "openai",
      input_per_1m_tokens: 2500000,
      output_per_1m_tokens: 10000000,
      source: "custom",
      created_at: "2024-01-01T00:00:00Z",
    },
    {
      id: "price-2",
      model: "claude-sonnet-4-20250514",
      provider: "anthropic",
      input_per_1m_tokens: 3000000,
      output_per_1m_tokens: 15000000,
      source: "default",
      created_at: "2024-01-01T00:00:00Z",
    },
  ],
  pagination: { limit: 100, has_more: false },
};

const mockBuiltInProviders = {
  data: [
    { name: "openai", provider_type: "open_ai", base_url: "https://api.openai.com/v1" },
    { name: "anthropic", provider_type: "anthropic", base_url: "https://api.anthropic.com" },
  ],
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
  http.get("*/admin/v1/organizations/acme-corp/projects/production-api", () =>
    HttpResponse.json(mockProject)
  ),
  http.get("*/admin/v1/organizations/acme-corp/projects/production-api/members", () =>
    HttpResponse.json(mockMembers)
  ),
  http.get("*/admin/v1/organizations/acme-corp/projects/production-api/api-keys", () =>
    HttpResponse.json(mockApiKeys)
  ),
  http.get("*/admin/v1/organizations/acme-corp/projects/production-api/dynamic-providers", () =>
    HttpResponse.json(mockProviders)
  ),
  http.get("*/admin/v1/organizations/acme-corp/projects/production-api/pricing", () =>
    HttpResponse.json(mockPricing)
  ),
  http.get("*/admin/v1/me/built-in-providers", () => HttpResponse.json(mockBuiltInProviders)),
  http.get("*/admin/v1/users", () =>
    HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } })
  ),
  http.get("*/admin/v1/organizations/acme-corp/teams", () =>
    HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } })
  ),
  http.patch("*/admin/v1/organizations/acme-corp/projects/production-api", async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>;
    return HttpResponse.json({ ...mockProject, ...body });
  }),
  http.post("*/admin/v1/dynamic-providers", async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>;
    return HttpResponse.json(
      {
        ...body,
        id: "new-prov",
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
        is_enabled: true,
        models: [],
      },
      { status: 201 }
    );
  }),
  http.post("*/admin/v1/dynamic-providers/:id/test", () =>
    HttpResponse.json({ status: "ok", message: "Connected successfully.", latency_ms: 180 })
  ),
  http.post("*/admin/v1/dynamic-providers/test-credentials", () =>
    HttpResponse.json({ status: "ok", message: "Connected successfully.", latency_ms: 200 })
  ),
  http.patch("*/admin/v1/dynamic-providers/:id", async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>;
    return HttpResponse.json({
      ...mockProviders.data[0],
      ...body,
      updated_at: new Date().toISOString(),
    });
  }),
  http.delete("*/admin/v1/dynamic-providers/:id", () => HttpResponse.json({})),
  http.post("*/admin/v1/organizations/acme-corp/projects/production-api/members", () =>
    HttpResponse.json({}, { status: 201 })
  ),
  http.delete("*/admin/v1/organizations/acme-corp/projects/production-api/members/:userId", () =>
    HttpResponse.json({})
  ),
  http.get("*/admin/v1/me/usage/summary", () => HttpResponse.json(mockUsageSummary)),
  http.get("*/admin/v1/me/usage/timeseries", () => HttpResponse.json({ data: [] })),
];

const meta: Meta<typeof ProjectDetailPage> = {
  title: "Pages/ProjectDetailPage",
  component: ProjectDetailPage,
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
              <MemoryRouter initialEntries={["/projects/acme-corp/production-api"]}>
                <Routes>
                  <Route path="/projects/:orgSlug/:projectSlug" element={<Story />} />
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

export const ProvidersEmpty: Story = {
  parameters: {
    msw: {
      handlers: [
        ...commonHandlers.filter((h) => !(h.info.path as string).includes("dynamic-providers")),
        http.get(
          "*/admin/v1/organizations/acme-corp/projects/production-api/dynamic-providers",
          () => HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } })
        ),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/acme-corp/projects/production-api", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockProject);
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
        http.get("*/admin/v1/organizations/acme-corp/projects/production-api", () =>
          HttpResponse.json({ error: "Not found" }, { status: 404 })
        ),
        ...commonHandlers.slice(1),
      ],
    },
  },
};
