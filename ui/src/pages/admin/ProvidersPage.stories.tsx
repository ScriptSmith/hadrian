import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import ProvidersPage from "./ProvidersPage";
import type {
  Organization,
  DynamicProvider,
  DynamicProviderListResponse,
  OrganizationListResponse,
  BuiltInProvidersResponse,
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

const mockBuiltInProviders: BuiltInProvidersResponse = {
  data: [
    { name: "openai", provider_type: "open_ai", base_url: "https://api.openai.com/v1" },
    { name: "anthropic", provider_type: "anthropic", base_url: "https://api.anthropic.com" },
  ],
};

const mockProviders: DynamicProvider[] = [
  {
    id: "dp-1",
    name: "production-openai",
    provider_type: "open_ai",
    base_url: "https://api.openai.com/v1",
    models: ["gpt-4o", "gpt-4o-mini", "o3-mini"],
    is_enabled: true,
    owner: { type: "organization", org_id: "org-123" },
    has_api_key: true,
    created_at: "2024-03-01T00:00:00Z",
    updated_at: "2024-06-15T10:30:00Z",
  },
  {
    id: "dp-2",
    name: "team-anthropic",
    provider_type: "anthropic",
    base_url: "https://api.anthropic.com",
    models: ["claude-sonnet-4-20250514", "claude-opus-4-20250514"],
    is_enabled: true,
    owner: { type: "team", team_id: "team-abc" },
    has_api_key: false,
    created_at: "2024-04-10T00:00:00Z",
    updated_at: "2024-05-20T14:00:00Z",
  },
  {
    id: "dp-3",
    name: "azure-gpt4",
    provider_type: "azure_openai",
    base_url: "https://acme.openai.azure.com",
    models: [],
    is_enabled: false,
    owner: { type: "project", project_id: "proj-xyz" },
    has_api_key: false,
    created_at: "2024-05-01T00:00:00Z",
    updated_at: "2024-05-01T00:00:00Z",
  },
  {
    id: "dp-4",
    name: "bedrock-us-east",
    provider_type: "bedrock",
    base_url: "https://bedrock-runtime.us-east-1.amazonaws.com",
    models: ["anthropic.claude-3-5-sonnet-20241022-v2:0", "amazon.nova-pro-v1:0"],
    is_enabled: true,
    owner: { type: "user", user_id: "user-001" },
    has_api_key: false,
    created_at: "2024-06-01T00:00:00Z",
    updated_at: "2024-06-10T09:00:00Z",
  },
  {
    id: "dp-5",
    name: "vertex-ai",
    provider_type: "vertex",
    base_url: "https://us-central1-aiplatform.googleapis.com",
    models: ["gemini-2.0-flash", "gemini-2.5-pro", "gemini-2.5-flash", "gemini-1.5-pro"],
    is_enabled: true,
    owner: { type: "organization", org_id: "org-123" },
    has_api_key: false,
    created_at: "2024-06-05T00:00:00Z",
    updated_at: "2024-06-12T16:00:00Z",
  },
];

const mockProvidersResponse: DynamicProviderListResponse = {
  data: mockProviders,
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const emptyProvidersResponse: DynamicProviderListResponse = {
  data: [],
  pagination: {
    limit: 100,
    has_more: false,
  },
};

const commonHandlers = [
  http.get("*/admin/v1/me/built-in-providers", () => {
    return HttpResponse.json(mockBuiltInProviders);
  }),
  http.post("*/admin/v1/dynamic-providers", async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>;
    const newProvider: DynamicProvider = {
      id: `dp-${Date.now()}`,
      name: body.name as string,
      provider_type: body.provider_type as string,
      base_url: body.base_url as string,
      models: (body.models as string[]) || [],
      is_enabled: true,
      owner: body.owner as DynamicProvider["owner"],
      has_api_key: !!body.api_key,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    return HttpResponse.json(newProvider, { status: 201 });
  }),
  http.patch("*/admin/v1/dynamic-providers/:id", async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>;
    const updated: DynamicProvider = {
      ...mockProviders[0],
      ...body,
      updated_at: new Date().toISOString(),
    };
    return HttpResponse.json(updated);
  }),
  http.delete("*/admin/v1/dynamic-providers/:id", () => {
    return HttpResponse.json({});
  }),
  http.post("*/admin/v1/dynamic-providers/:id/test", () => {
    return HttpResponse.json({
      status: "ok",
      message: "Connected successfully. 12 models available.",
      latency_ms: 142,
    });
  }),
  http.post("*/admin/v1/dynamic-providers/test-credentials", () => {
    return HttpResponse.json({
      status: "ok",
      message: "Connected successfully. 8 models available.",
      latency_ms: 230,
    });
  }),
];

const meta: Meta<typeof ProvidersPage> = {
  title: "Admin/ProvidersPage",
  component: ProvidersPage,
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
            <MemoryRouter initialEntries={["/admin/providers"]}>
              <Routes>
                <Route path="/admin/providers" element={<Story />} />
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
        http.get("*/admin/v1/organizations/acme-corp/dynamic-providers", () => {
          return HttpResponse.json(mockProvidersResponse);
        }),
        http.get("*/admin/v1/organizations/stark-industries/dynamic-providers", () => {
          return HttpResponse.json(emptyProvidersResponse);
        }),
        ...commonHandlers,
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
        http.get("*/admin/v1/organizations/acme-corp/dynamic-providers", () => {
          return HttpResponse.json(emptyProvidersResponse);
        }),
        http.get("*/admin/v1/organizations/stark-industries/dynamic-providers", () => {
          return HttpResponse.json(emptyProvidersResponse);
        }),
        ...commonHandlers,
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
        http.get("*/admin/v1/organizations/:orgSlug/dynamic-providers", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockProvidersResponse);
        }),
        ...commonHandlers,
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
        http.get("*/admin/v1/organizations/acme-corp/dynamic-providers", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        ...commonHandlers,
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
            pagination: { limit: 100, has_more: false },
          });
        }),
        ...commonHandlers,
      ],
    },
  },
};

export const ManyProviders: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations", () => {
          return HttpResponse.json(mockOrgsResponse);
        }),
        http.get("*/admin/v1/organizations/acme-corp/dynamic-providers", () => {
          const manyProviders: DynamicProvider[] = Array.from({ length: 12 }, (_, i) => ({
            id: `dp-${i + 1}`,
            name: `provider-${i + 1}`,
            provider_type: ["open_ai", "anthropic", "azure_openai", "bedrock", "vertex"][i % 5],
            base_url: `https://api-${i + 1}.example.com/v1`,
            models: i % 3 === 0 ? [] : [`model-${i}-a`, `model-${i}-b`],
            is_enabled: i % 4 !== 0,
            owner: { type: "organization" as const, org_id: "org-123" },
            has_api_key: false,
            created_at: "2024-01-01T00:00:00Z",
            updated_at: "2024-06-15T10:30:00Z",
          }));
          return HttpResponse.json({
            data: manyProviders,
            pagination: { limit: 100, has_more: false },
          });
        }),
        ...commonHandlers,
      ],
    },
  },
};
