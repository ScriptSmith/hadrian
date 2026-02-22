import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import OrganizationDetailPage from "./OrganizationDetailPage";
import type {
  Organization,
  User,
  Project,
  ApiKey,
  DynamicProvider,
  DbModelPricing,
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

const mockOrg: Organization = {
  id: "org-123",
  slug: "acme-corp",
  name: "Acme Corporation",
  created_at: "2024-01-01T00:00:00Z",
  updated_at: "2024-06-15T10:30:00Z",
};

const mockProjects: Project[] = [
  {
    id: "proj-001",
    name: "Production API",
    slug: "production-api",
    org_id: "org-123",
    team_id: null,
    created_at: "2024-02-01T09:00:00Z",
    updated_at: "2024-06-10T14:00:00Z",
  },
  {
    id: "proj-002",
    name: "Staging Environment",
    slug: "staging-env",
    org_id: "org-123",
    team_id: "team-abc",
    created_at: "2024-03-15T11:00:00Z",
    updated_at: "2024-05-20T16:30:00Z",
  },
  {
    id: "proj-003",
    name: "Research Playground",
    slug: "research-playground",
    org_id: "org-123",
    team_id: null,
    created_at: "2024-04-22T08:00:00Z",
    updated_at: "2024-04-22T08:00:00Z",
  },
];

const mockMembers: User[] = [
  {
    id: "usr-001",
    external_id: "auth0|alice",
    name: "Alice Johnson",
    email: "alice@acme-corp.com",
    created_at: "2024-01-10T09:00:00Z",
    updated_at: "2024-06-15T14:30:00Z",
  },
  {
    id: "usr-002",
    external_id: "auth0|bob",
    name: "Bob Martinez",
    email: "bob@acme-corp.com",
    created_at: "2024-02-14T11:20:00Z",
    updated_at: "2024-02-14T11:20:00Z",
  },
  {
    id: "usr-003",
    external_id: "okta|charlie",
    name: null,
    email: "charlie@acme-corp.com",
    created_at: "2024-03-05T16:45:00Z",
    updated_at: "2024-05-20T08:10:00Z",
  },
];

const mockApiKeys: ApiKey[] = [
  {
    id: "key-001",
    name: "Production API Key",
    key_prefix: "sk-prod-abc",
    owner: { type: "organization", org_id: "org-123" },
    created_at: "2024-02-01T09:00:00Z",
    revoked_at: null,
    expires_at: "2025-02-01T09:00:00Z",
    last_used_at: "2024-06-15T10:00:00Z",
  },
  {
    id: "key-002",
    name: "Staging Key",
    key_prefix: "sk-stg-xyz",
    owner: { type: "organization", org_id: "org-123" },
    created_at: "2024-03-10T14:00:00Z",
    revoked_at: null,
    expires_at: null,
    last_used_at: "2024-06-14T22:00:00Z",
  },
  {
    id: "key-003",
    name: "Deprecated Key",
    key_prefix: "sk-old-789",
    owner: { type: "organization", org_id: "org-123" },
    created_at: "2024-01-05T08:00:00Z",
    revoked_at: "2024-05-01T12:00:00Z",
    expires_at: null,
    last_used_at: "2024-04-30T18:00:00Z",
  },
];

const mockProviders: DynamicProvider[] = [
  {
    id: "dp-1",
    name: "Production OpenAI",
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
    name: "Anthropic Claude",
    provider_type: "anthropic",
    base_url: "https://api.anthropic.com",
    models: ["claude-sonnet-4-20250514"],
    is_enabled: true,
    owner: { type: "organization", org_id: "org-123" },
    has_api_key: false,
    created_at: "2024-04-10T00:00:00Z",
    updated_at: "2024-05-20T14:00:00Z",
  },
];

const mockPricing: DbModelPricing[] = [
  {
    id: "price-001",
    model: "gpt-4o",
    provider: "open_ai",
    input_per_1m_tokens: 2_500_000,
    output_per_1m_tokens: 10_000_000,
    source: "manual",
    owner: { type: "organization", org_id: "org-123" },
    created_at: "2024-03-01T00:00:00Z",
  },
  {
    id: "price-002",
    model: "claude-sonnet-4-20250514",
    provider: "anthropic",
    input_per_1m_tokens: 3_000_000,
    output_per_1m_tokens: 15_000_000,
    source: "provider_api",
    owner: { type: "organization", org_id: "org-123" },
    created_at: "2024-04-01T00:00:00Z",
  },
];

const mockAllUsers: User[] = [
  ...mockMembers,
  {
    id: "usr-004",
    external_id: "saml|diana",
    name: "Diana Chen",
    email: "diana@acme-corp.com",
    created_at: "2024-04-22T13:00:00Z",
    updated_at: "2024-04-22T13:00:00Z",
  },
  {
    id: "usr-005",
    external_id: "oidc|eve",
    name: "Eve Taylor",
    email: "eve@acme-corp.com",
    created_at: "2024-05-01T07:30:00Z",
    updated_at: "2024-05-01T07:30:00Z",
  },
];

const orgSlug = "acme-corp";

const createDecorator = () => (Story: React.ComponentType) => {
  const queryClient = createQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <ConfirmDialogProvider>
          <MemoryRouter initialEntries={[`/admin/organizations/${orgSlug}`]}>
            <Routes>
              <Route path="/admin/organizations/:slug" element={<Story />} />
              <Route path="/admin/organizations" element={<div>Organizations List Page</div>} />
              <Route
                path="/admin/organizations/:slug/projects/new"
                element={<div>New Project Page</div>}
              />
              <Route
                path="/admin/organizations/:slug/projects/:projectSlug"
                element={<div>Project Detail Page</div>}
              />
              <Route
                path="/admin/organizations/:slug/sso-config"
                element={<div>SSO Config Page</div>}
              />
              <Route
                path="/admin/organizations/:slug/scim-config"
                element={<div>SCIM Config Page</div>}
              />
              <Route
                path="/admin/organizations/:slug/sso-group-mappings"
                element={<div>SSO Group Mappings Page</div>}
              />
              <Route
                path="/admin/organizations/:slug/rbac-policies"
                element={<div>RBAC Policies Page</div>}
              />
            </Routes>
          </MemoryRouter>
        </ConfirmDialogProvider>
      </ToastProvider>
    </QueryClientProvider>
  );
};

const defaultHandlers = [
  http.get("*/admin/v1/organizations/:slug", () => {
    return HttpResponse.json(mockOrg);
  }),
  http.get("*/admin/v1/organizations/:orgSlug/projects", () => {
    return HttpResponse.json({ data: mockProjects, pagination: { limit: 100, has_more: false } });
  }),
  http.get("*/admin/v1/organizations/:orgSlug/members", () => {
    return HttpResponse.json({ data: mockMembers, pagination: { limit: 100, has_more: false } });
  }),
  http.get("*/admin/v1/organizations/:orgSlug/api-keys", () => {
    return HttpResponse.json({ data: mockApiKeys, pagination: { limit: 100, has_more: false } });
  }),
  http.get("*/admin/v1/organizations/:orgSlug/dynamic-providers", () => {
    return HttpResponse.json({
      data: mockProviders,
      pagination: { limit: 100, has_more: false },
    });
  }),
  http.get("*/admin/v1/organizations/:orgSlug/model-pricing", () => {
    return HttpResponse.json({
      data: mockPricing,
      pagination: { limit: 100, has_more: false },
    });
  }),
  http.get("*/admin/v1/users", () => {
    return HttpResponse.json({ data: mockAllUsers, pagination: { limit: 100, has_more: false } });
  }),
  http.patch("*/admin/v1/organizations/:slug", async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>;
    return HttpResponse.json({ ...mockOrg, ...body, updated_at: new Date().toISOString() });
  }),
  http.post("*/admin/v1/organizations/:orgSlug/members", () => {
    return HttpResponse.json({}, { status: 201 });
  }),
  http.delete("*/admin/v1/organizations/:orgSlug/members/:userId", () => {
    return HttpResponse.json({});
  }),
  // Usage endpoints
  http.get("*/admin/v1/organizations/:slug/usage", () => {
    return HttpResponse.json({
      total_requests: 15420,
      total_input_tokens: 2500000,
      total_output_tokens: 1200000,
      total_cost_microcents: 4500000000,
    });
  }),
  http.get("*/admin/v1/organizations/:slug/usage/*", () => {
    return HttpResponse.json({ data: [] });
  }),
];

const meta: Meta<typeof OrganizationDetailPage> = {
  title: "Admin/OrganizationDetailPage",
  component: OrganizationDetailPage,
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
      handlers: defaultHandlers,
    },
  },
};

export const Loading: Story = {
  decorators: [createDecorator()],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:slug", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockOrg);
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
        http.get("*/admin/v1/organizations/:slug", () => {
          return HttpResponse.json(mockOrg);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/projects", () => {
          return HttpResponse.json({
            data: [],
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/organizations/:orgSlug/members", () => {
          return HttpResponse.json({
            data: [],
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/organizations/:orgSlug/api-keys", () => {
          return HttpResponse.json({
            data: [],
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/organizations/:orgSlug/dynamic-providers", () => {
          return HttpResponse.json({
            data: [],
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/organizations/:orgSlug/model-pricing", () => {
          return HttpResponse.json({
            data: [],
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json({
            data: [],
            pagination: { limit: 100, has_more: false },
          });
        }),
        http.get("*/admin/v1/organizations/:slug/usage", () => {
          return HttpResponse.json({
            total_requests: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_microcents: 0,
          });
        }),
        http.get("*/admin/v1/organizations/:slug/usage/*", () => {
          return HttpResponse.json({ data: [] });
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
        http.get("*/admin/v1/organizations/:slug", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "Organization not found" } },
            { status: 404 }
          );
        }),
      ],
    },
  },
};
