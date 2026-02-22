import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import ProjectDetailPage from "./ProjectDetailPage";
import type {
  Project,
  User,
  ApiKey,
  DynamicProvider,
  DbModelPricing,
  UserListResponse,
  ApiKeyListResponse,
  DynamicProviderListResponse,
  Team,
  TeamListResponse,
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

const mockProject: Project = {
  id: "proj-001",
  slug: "production-api",
  name: "Production API",
  org_id: "org-123",
  team_id: "team-001",
  created_at: "2024-01-20T09:00:00Z",
  updated_at: "2024-06-15T14:30:00Z",
};

const mockProjectNoTeam: Project = {
  id: "proj-002",
  slug: "sandbox",
  name: "Sandbox",
  org_id: "org-123",
  team_id: null,
  created_at: "2024-04-22T13:00:00Z",
  updated_at: "2024-04-22T13:00:00Z",
};

const mockTeams: Team[] = [
  {
    id: "team-001",
    slug: "engineering",
    name: "Engineering",
    org_id: "org-123",
    created_at: "2024-01-15T00:00:00Z",
    updated_at: "2024-01-15T00:00:00Z",
  },
  {
    id: "team-002",
    slug: "data-science",
    name: "Data Science",
    org_id: "org-123",
    created_at: "2024-02-20T00:00:00Z",
    updated_at: "2024-02-20T00:00:00Z",
  },
];

const mockTeamsResponse: TeamListResponse = {
  data: mockTeams,
  pagination: { limit: 100, has_more: false },
};

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

const mockMembersResponse: UserListResponse = {
  data: mockMembers,
  pagination: { limit: 25, has_more: false },
};

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
    name: "Eve Park",
    email: "eve@acme-corp.com",
    created_at: "2024-05-01T07:30:00Z",
    updated_at: "2024-05-01T07:30:00Z",
  },
];

const mockAllUsersResponse: UserListResponse = {
  data: mockAllUsers,
  pagination: { limit: 100, has_more: false },
};

const mockApiKeys: ApiKey[] = [
  {
    id: "key-001",
    name: "CI/CD Pipeline Key",
    key_prefix: "sk-cicd-abc",
    owner: { type: "project", project_id: "proj-001" },
    revoked_at: null,
    expires_at: "2025-06-01T00:00:00Z",
    budget_limit_cents: 10000,
    budget_period: "monthly",
    created_at: "2024-02-01T10:00:00Z",
    last_used_at: "2024-06-15T13:45:00Z",
  },
  {
    id: "key-002",
    name: "Staging Key",
    key_prefix: "sk-stag-xyz",
    owner: { type: "project", project_id: "proj-001" },
    revoked_at: null,
    expires_at: null,
    budget_limit_cents: null,
    budget_period: null,
    created_at: "2024-03-10T08:00:00Z",
    last_used_at: "2024-06-14T20:00:00Z",
  },
];

const mockApiKeysResponse: ApiKeyListResponse = {
  data: mockApiKeys,
  pagination: { limit: 25, has_more: false },
};

const mockProviders: DynamicProvider[] = [
  {
    id: "prov-001",
    name: "Project OpenAI",
    provider_type: "open_ai",
    base_url: "https://api.openai.com/v1",
    is_enabled: true,
    models: ["gpt-4", "gpt-4-turbo", "gpt-3.5-turbo"],
    owner: { type: "project", project_id: "proj-001" },
    created_at: "2024-01-25T00:00:00Z",
    updated_at: "2024-06-01T00:00:00Z",
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
    owner: { type: "project", project_id: "proj-001" },
    created_at: "2024-01-20T00:00:00Z",
    updated_at: "2024-01-20T00:00:00Z",
  },
];

const emptyListResponse = {
  data: [],
  pagination: { limit: 25, has_more: false },
};

const usageSummaryResponse = {
  total_requests: 3200,
  total_input_tokens: 920000,
  total_output_tokens: 480000,
  total_cost_microcents: 1650000000,
  period_start: "2024-06-01T00:00:00Z",
  period_end: "2024-06-15T23:59:59Z",
};

const createUsageHandlers = () => [
  http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/usage", () => {
    return HttpResponse.json(usageSummaryResponse);
  }),
  http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/usage/by-model", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/usage/by-provider", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/usage/by-date", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/usage/by-date-model", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/usage/by-date-provider", () => {
    return HttpResponse.json({ data: [] });
  }),
  http.get(
    "*/admin/v1/organizations/:orgSlug/projects/:projectSlug/usage/by-date-pricing-source",
    () => {
      return HttpResponse.json({ data: [] });
    }
  ),
  http.get(
    "*/admin/v1/organizations/:orgSlug/projects/:projectSlug/usage/by-pricing-source",
    () => {
      return HttpResponse.json({ data: [] });
    }
  ),
  http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/usage/forecast", () => {
    return HttpResponse.json({});
  }),
];

const createDecorator = (orgSlug: string, projectSlug: string) => (Story: React.ComponentType) => {
  const queryClient = createQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <ConfirmDialogProvider>
          <MemoryRouter
            initialEntries={[`/admin/organizations/${orgSlug}/projects/${projectSlug}`]}
          >
            <Routes>
              <Route
                path="/admin/organizations/:orgSlug/projects/:projectSlug"
                element={<Story />}
              />
              <Route
                path="/admin/organizations/:orgSlug"
                element={<div>Organization Detail Page</div>}
              />
            </Routes>
          </MemoryRouter>
        </ConfirmDialogProvider>
      </ToastProvider>
    </QueryClientProvider>
  );
};

const meta: Meta<typeof ProjectDetailPage> = {
  title: "Admin/ProjectDetailPage",
  component: ProjectDetailPage,
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
  decorators: [createDecorator("acme-corp", "production-api")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug", () => {
          return HttpResponse.json(mockProject);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/members", () => {
          return HttpResponse.json(mockMembersResponse);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/api-keys", () => {
          return HttpResponse.json(mockApiKeysResponse);
        }),
        http.get(
          "*/admin/v1/organizations/:orgSlug/projects/:projectSlug/dynamic-providers",
          () => {
            return HttpResponse.json(mockProvidersResponse);
          }
        ),
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/model-pricing", () => {
          return HttpResponse.json({
            data: mockPricing,
            pagination: { limit: 25, has_more: false },
          });
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(mockAllUsersResponse);
        }),
        http.patch(
          "*/admin/v1/organizations/:orgSlug/projects/:projectSlug",
          async ({ request }) => {
            const body = (await request.json()) as Record<string, unknown>;
            return HttpResponse.json({
              ...mockProject,
              name: (body.name as string) || mockProject.name,
              updated_at: new Date().toISOString(),
            });
          }
        ),
        http.post("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/members", () => {
          return HttpResponse.json({}, { status: 201 });
        }),
        http.delete(
          "*/admin/v1/organizations/:orgSlug/projects/:projectSlug/members/:userId",
          () => {
            return HttpResponse.json({});
          }
        ),
        ...createUsageHandlers(),
      ],
    },
  },
};

export const Loading: Story = {
  decorators: [createDecorator("acme-corp", "production-api")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockProject);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  decorators: [createDecorator("acme-corp", "production-api")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug", () => {
          return HttpResponse.json(mockProject);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/members", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/api-keys", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get(
          "*/admin/v1/organizations/:orgSlug/projects/:projectSlug/dynamic-providers",
          () => {
            return HttpResponse.json(emptyListResponse);
          }
        ),
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/model-pricing", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(mockAllUsersResponse);
        }),
        http.post("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/members", () => {
          return HttpResponse.json({}, { status: 201 });
        }),
        ...createUsageHandlers(),
      ],
    },
  },
};

export const Error: Story = {
  decorators: [createDecorator("acme-corp", "missing-project")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "Project not found" } },
            { status: 404 }
          );
        }),
      ],
    },
  },
};

export const NoTeam: Story = {
  decorators: [createDecorator("acme-corp", "sandbox")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug", () => {
          return HttpResponse.json(mockProjectNoTeam);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/members", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/api-keys", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get(
          "*/admin/v1/organizations/:orgSlug/projects/:projectSlug/dynamic-providers",
          () => {
            return HttpResponse.json(emptyListResponse);
          }
        ),
        http.get("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/model-pricing", () => {
          return HttpResponse.json(emptyListResponse);
        }),
        http.get("*/admin/v1/organizations/:orgSlug/teams", () => {
          return HttpResponse.json(mockTeamsResponse);
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(mockAllUsersResponse);
        }),
        http.patch(
          "*/admin/v1/organizations/:orgSlug/projects/:projectSlug",
          async ({ request }) => {
            const body = (await request.json()) as Record<string, unknown>;
            return HttpResponse.json({
              ...mockProjectNoTeam,
              name: (body.name as string) || mockProjectNoTeam.name,
              updated_at: new Date().toISOString(),
            });
          }
        ),
        http.post("*/admin/v1/organizations/:orgSlug/projects/:projectSlug/members", () => {
          return HttpResponse.json({}, { status: 201 });
        }),
        ...createUsageHandlers(),
      ],
    },
  },
};
