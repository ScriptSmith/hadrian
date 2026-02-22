import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import UserDetailPage from "./UserDetailPage";
import type {
  User,
  ApiKey,
  DynamicProvider,
  DbModelPricing,
  ApiKeyListResponse,
  DynamicProviderListResponse,
  SessionListResponse,
} from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";

const createQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        staleTime: Infinity,
      },
    },
  });

const mockUser: User = {
  id: "usr-001",
  external_id: "auth0|alice-johnson",
  name: "Alice Johnson",
  email: "alice@acme-corp.com",
  created_at: "2024-01-10T09:00:00Z",
  updated_at: "2024-06-15T14:30:00Z",
};

const mockUserNoName: User = {
  id: "usr-002",
  external_id: "okta|anonymous-user",
  name: null,
  email: "anon@acme-corp.com",
  created_at: "2024-03-05T16:45:00Z",
  updated_at: "2024-05-20T08:10:00Z",
};

const mockApiKeys: ApiKey[] = [
  {
    id: "key-001",
    name: "Production API Key",
    key_prefix: "sk-prod-abc",
    owner: { type: "user", user_id: "usr-001" },
    revoked_at: null,
    expires_at: "2025-01-10T09:00:00Z",
    budget_limit_cents: 5000,
    budget_period: "monthly",
    created_at: "2024-01-15T10:00:00Z",
    last_used_at: "2024-06-15T14:00:00Z",
  },
  {
    id: "key-002",
    name: "Development Key",
    key_prefix: "sk-dev-xyz",
    owner: { type: "user", user_id: "usr-001" },
    revoked_at: null,
    expires_at: null,
    budget_limit_cents: null,
    budget_period: null,
    created_at: "2024-02-20T08:30:00Z",
    last_used_at: "2024-06-14T22:15:00Z",
  },
  {
    id: "key-003",
    name: "Deprecated Key",
    key_prefix: "sk-old-def",
    owner: { type: "user", user_id: "usr-001" },
    revoked_at: "2024-05-01T00:00:00Z",
    expires_at: null,
    budget_limit_cents: 1000,
    budget_period: "daily",
    created_at: "2024-01-05T12:00:00Z",
    last_used_at: "2024-04-30T23:59:00Z",
  },
];

const mockApiKeysResponse: ApiKeyListResponse = {
  data: mockApiKeys,
  pagination: { limit: 25, has_more: false },
};

const mockProviders: DynamicProvider[] = [
  {
    id: "prov-001",
    name: "Custom OpenAI",
    provider_type: "open_ai",
    base_url: "https://api.openai.com/v1",
    is_enabled: true,
    models: ["gpt-4", "gpt-3.5-turbo"],
    owner: { type: "user", user_id: "usr-001" },
    created_at: "2024-02-01T00:00:00Z",
    updated_at: "2024-06-01T00:00:00Z",
  },
  {
    id: "prov-002",
    name: "Azure Endpoint",
    provider_type: "azure_openai",
    base_url: "https://acme.openai.azure.com",
    is_enabled: false,
    models: ["gpt-4-turbo"],
    owner: { type: "user", user_id: "usr-001" },
    created_at: "2024-03-15T00:00:00Z",
    updated_at: "2024-03-15T00:00:00Z",
  },
];

const mockProvidersResponse: DynamicProviderListResponse = {
  data: mockProviders,
  pagination: { limit: 25, has_more: false },
};

const mockPricing: DbModelPricing[] = [
  {
    id: "price-001",
    model: "gpt-4-turbo",
    provider: "openai",
    input_per_1m_tokens: 10000000,
    output_per_1m_tokens: 30000000,
    source: "config",
    owner: { type: "user", user_id: "usr-001" },
    created_at: "2024-01-20T00:00:00Z",
    updated_at: "2024-01-20T00:00:00Z",
  },
  {
    id: "price-002",
    model: "claude-3-opus",
    provider: "anthropic",
    input_per_1m_tokens: 15000000,
    output_per_1m_tokens: 75000000,
    source: "database",
    owner: { type: "user", user_id: "usr-001" },
    created_at: "2024-02-15T00:00:00Z",
    updated_at: "2024-02-15T00:00:00Z",
  },
];

const mockSessionsResponse: SessionListResponse = {
  enhanced_enabled: true,
  data: [
    {
      id: "sess-001",
      created_at: "2024-06-15T08:00:00Z",
      expires_at: "2024-06-16T08:00:00Z",
      last_activity: "2024-06-15T14:30:00Z",
      device: {
        device_description: "Chrome 126 on macOS",
        device_id: "abc123",
        ip_address: "192.168.1.42",
        user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
      },
    },
    {
      id: "sess-002",
      created_at: "2024-06-14T10:00:00Z",
      expires_at: "2024-06-15T10:00:00Z",
      last_activity: "2024-06-14T18:00:00Z",
      device: {
        device_description: "Firefox 127 on Linux",
        device_id: "def456",
        ip_address: "10.0.0.15",
        user_agent: "Mozilla/5.0 (X11; Linux x86_64; rv:127.0) Gecko/20100101 Firefox/127.0",
      },
    },
  ],
};

const usageSummaryResponse = {
  total_requests: 1520,
  total_input_tokens: 450000,
  total_output_tokens: 210000,
  total_cost_microcents: 850000000,
  period_start: "2024-06-01T00:00:00Z",
  period_end: "2024-06-15T23:59:59Z",
};

const emptyListResponse = {
  data: [],
  pagination: { limit: 25, has_more: false },
};

const createUsageHandlers = () => [
  http.get("*/admin/v1/users/:userId/usage", () => {
    return HttpResponse.json(usageSummaryResponse);
  }),
  http.get("*/admin/v1/users/:userId/usage/by-model", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/users/:userId/usage/by-provider", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/users/:userId/usage/by-date", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/users/:userId/usage/by-date-model", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/users/:userId/usage/by-date-provider", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/users/:userId/usage/by-date-pricing-source", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/users/:userId/usage/by-pricing-source", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/users/:userId/usage/forecast", () => {
    return HttpResponse.json({});
  }),
];

const createDecorator = (userId: string) => (Story: React.ComponentType) => {
  const queryClient = createQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <MemoryRouter initialEntries={[`/admin/users/${userId}`]}>
          <Routes>
            <Route path="/admin/users/:userId" element={<Story />} />
            <Route path="/admin/users" element={<div>Users List Page</div>} />
          </Routes>
        </MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>
  );
};

const meta: Meta<typeof UserDetailPage> = {
  title: "Admin/UserDetailPage",
  component: UserDetailPage,
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
  decorators: [createDecorator("usr-001")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/users/:userId", () => {
          return HttpResponse.json(mockUser);
        }),
        http.get("*/admin/v1/users/:userId/api-keys", () => {
          return HttpResponse.json(mockApiKeysResponse);
        }),
        http.get("*/admin/v1/users/:userId/dynamic-providers", () => {
          return HttpResponse.json(mockProvidersResponse);
        }),
        http.get("*/admin/v1/users/:userId/model-pricing", () => {
          return HttpResponse.json({
            data: mockPricing,
            pagination: { limit: 25, has_more: false },
          });
        }),
        http.get("*/admin/v1/users/:userId/sessions", () => {
          return HttpResponse.json(mockSessionsResponse);
        }),
        http.patch("*/admin/v1/users/:userId", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          return HttpResponse.json({
            ...mockUser,
            name: (body.name as string) ?? mockUser.name,
            email: (body.email as string) ?? mockUser.email,
            updated_at: new Date().toISOString(),
          });
        }),
        ...createUsageHandlers(),
      ],
    },
  },
};

export const Loading: Story = {
  decorators: [createDecorator("usr-001")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/users/:userId", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockUser);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  decorators: [createDecorator("usr-001")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/users/:userId", () => {
          return HttpResponse.json(mockUser);
        }),
        http.get("*/admin/v1/users/:userId/api-keys", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/users/:userId/dynamic-providers", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/users/:userId/model-pricing", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/users/:userId/sessions", () => {
          return HttpResponse.json({ enhanced_enabled: true, data: [] });
        }),
        http.patch("*/admin/v1/users/:userId", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          return HttpResponse.json({
            ...mockUser,
            name: (body.name as string) ?? mockUser.name,
            email: (body.email as string) ?? mockUser.email,
            updated_at: new Date().toISOString(),
          });
        }),
        ...createUsageHandlers(),
      ],
    },
  },
};

export const Error: Story = {
  decorators: [createDecorator("usr-999")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/users/:userId", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "User not found" } },
            { status: 404 }
          );
        }),
      ],
    },
  },
};

export const UserWithNoName: Story = {
  decorators: [createDecorator("usr-002")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/users/:userId", () => {
          return HttpResponse.json(mockUserNoName);
        }),
        http.get("*/admin/v1/users/:userId/api-keys", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/users/:userId/dynamic-providers", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/users/:userId/model-pricing", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/users/:userId/sessions", () => {
          return HttpResponse.json({ enhanced_enabled: false, data: [] });
        }),
        ...createUsageHandlers(),
      ],
    },
  },
};
