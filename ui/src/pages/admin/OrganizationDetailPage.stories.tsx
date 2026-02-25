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

const daysAgo = (d: number) => new Date(Date.now() - d * 86_400_000).toISOString();

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
  created_at: daysAgo(180),
  updated_at: daysAgo(5),
};

const mockProjects: Project[] = [
  {
    id: "proj-001",
    name: "Production API",
    slug: "production-api",
    org_id: "org-123",
    team_id: null,
    created_at: daysAgo(160),
    updated_at: daysAgo(15),
  },
  {
    id: "proj-002",
    name: "Staging Environment",
    slug: "staging-env",
    org_id: "org-123",
    team_id: "team-abc",
    created_at: daysAgo(120),
    updated_at: daysAgo(40),
  },
  {
    id: "proj-003",
    name: "Research Playground",
    slug: "research-playground",
    org_id: "org-123",
    team_id: null,
    created_at: daysAgo(90),
    updated_at: daysAgo(90),
  },
];

const mockMembers: User[] = [
  {
    id: "usr-001",
    external_id: "auth0|alice",
    name: "Alice Johnson",
    email: "alice@acme-corp.com",
    created_at: daysAgo(170),
    updated_at: daysAgo(10),
  },
  {
    id: "usr-002",
    external_id: "auth0|bob",
    name: "Bob Martinez",
    email: "bob@acme-corp.com",
    created_at: daysAgo(130),
    updated_at: daysAgo(130),
  },
  {
    id: "usr-003",
    external_id: "okta|charlie",
    name: null,
    email: "charlie@acme-corp.com",
    created_at: daysAgo(110),
    updated_at: daysAgo(35),
  },
];

const mockApiKeys: ApiKey[] = [
  {
    id: "key-001",
    name: "Production API Key",
    key_prefix: "sk-prod-abc",
    owner: { type: "organization", org_id: "org-123" },
    created_at: daysAgo(150),
    revoked_at: null,
    expires_at: daysAgo(-180),
    last_used_at: daysAgo(0),
  },
  {
    id: "key-002",
    name: "Staging Key",
    key_prefix: "sk-stg-xyz",
    owner: { type: "organization", org_id: "org-123" },
    created_at: daysAgo(100),
    revoked_at: null,
    expires_at: null,
    last_used_at: daysAgo(1),
  },
  {
    id: "key-003",
    name: "Deprecated Key",
    key_prefix: "sk-old-789",
    owner: { type: "organization", org_id: "org-123" },
    created_at: daysAgo(170),
    revoked_at: daysAgo(60),
    expires_at: null,
    last_used_at: daysAgo(61),
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
    created_at: daysAgo(140),
    updated_at: daysAgo(5),
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
    created_at: daysAgo(100),
    updated_at: daysAgo(30),
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
    created_at: daysAgo(140),
  },
  {
    id: "price-002",
    model: "claude-sonnet-4-20250514",
    provider: "anthropic",
    input_per_1m_tokens: 3_000_000,
    output_per_1m_tokens: 15_000_000,
    source: "provider_api",
    owner: { type: "organization", org_id: "org-123" },
    created_at: daysAgo(100),
  },
];

const mockAllUsers: User[] = [
  ...mockMembers,
  {
    id: "usr-004",
    external_id: "saml|diana",
    name: "Diana Chen",
    email: "diana@acme-corp.com",
    created_at: daysAgo(80),
    updated_at: daysAgo(80),
  },
  {
    id: "usr-005",
    external_id: "oidc|eve",
    name: "Eve Taylor",
    email: "eve@acme-corp.com",
    created_at: daysAgo(60),
    updated_at: daysAgo(60),
  },
];

// --- Usage mock data ---

const MODELS = ["gpt-4o", "claude-opus-4-6", "gpt-4o-mini", "claude-haiku-4-5"];
const PROVIDERS_LIST = ["openai", "anthropic"];

const mockUsageSummary = {
  total_cost: 42.57,
  total_tokens: 1_250_000,
  input_tokens: 820_000,
  output_tokens: 430_000,
  request_count: 3_421,
  first_request_at: daysAgo(30),
  last_request_at: daysAgo(0),
  image_count: 156,
  audio_seconds: 4_320,
  character_count: 85_000,
};

const mockUsageByDate = Array.from({ length: 30 }, (_, i) => {
  const inputTokens = Math.floor(Math.random() * 30000 + 8000);
  const outputTokens = Math.floor(Math.random() * 20000 + 5000);
  return {
    date: `2026-01-${String(i + 1).padStart(2, "0")}`,
    total_cost: Math.random() * 3 + 0.5,
    total_tokens: inputTokens + outputTokens,
    input_tokens: inputTokens,
    output_tokens: outputTokens,
    request_count: Math.floor(Math.random() * 200 + 50),
    image_count: Math.floor(Math.random() * 10),
    audio_seconds: Math.floor(Math.random() * 200),
    character_count: Math.floor(Math.random() * 5000),
  };
});

const mockUsageByModel = [
  {
    model: "gpt-4o",
    total_cost: 22.3,
    total_tokens: 400000,
    input_tokens: 260000,
    output_tokens: 140000,
    request_count: 1200,
    image_count: 80,
    audio_seconds: 1500,
    character_count: 35000,
  },
  {
    model: "claude-opus-4-6",
    total_cost: 15.2,
    total_tokens: 350000,
    input_tokens: 230000,
    output_tokens: 120000,
    request_count: 900,
    image_count: 50,
    audio_seconds: 1800,
    character_count: 30000,
  },
  {
    model: "gpt-4o-mini",
    total_cost: 3.1,
    total_tokens: 300000,
    input_tokens: 200000,
    output_tokens: 100000,
    request_count: 800,
    image_count: 20,
    audio_seconds: 720,
    character_count: 15000,
  },
  {
    model: "claude-haiku-4-5",
    total_cost: 1.97,
    total_tokens: 200000,
    input_tokens: 130000,
    output_tokens: 70000,
    request_count: 521,
    image_count: 6,
    audio_seconds: 300,
    character_count: 5000,
  },
];

const mockUsageByProvider = [
  {
    provider: "openai",
    total_cost: 25.4,
    total_tokens: 700000,
    input_tokens: 460000,
    output_tokens: 240000,
    request_count: 2000,
    image_count: 100,
    audio_seconds: 2220,
    character_count: 50000,
  },
  {
    provider: "anthropic",
    total_cost: 17.17,
    total_tokens: 550000,
    input_tokens: 360000,
    output_tokens: 190000,
    request_count: 1421,
    image_count: 56,
    audio_seconds: 2100,
    character_count: 35000,
  },
];

const mockUsageByDateModel = Array.from({ length: 30 }, (_, i) =>
  MODELS.map((model) => {
    const inputTokens = Math.floor(Math.random() * 8000 + 2000);
    const outputTokens = Math.floor(Math.random() * 5000 + 1000);
    return {
      date: `2026-01-${String(i + 1).padStart(2, "0")}`,
      model,
      total_cost: Math.random() * 1.5 + 0.1,
      total_tokens: inputTokens + outputTokens,
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      request_count: Math.floor(Math.random() * 60 + 10),
      image_count: Math.floor(Math.random() * 5),
      audio_seconds: Math.floor(Math.random() * 60),
      character_count: Math.floor(Math.random() * 2000),
    };
  })
).flat();

const mockUsageByDateProvider = Array.from({ length: 30 }, (_, i) =>
  PROVIDERS_LIST.map((provider) => {
    const inputTokens = Math.floor(Math.random() * 15000 + 5000);
    const outputTokens = Math.floor(Math.random() * 10000 + 3000);
    return {
      date: `2026-01-${String(i + 1).padStart(2, "0")}`,
      provider,
      total_cost: Math.random() * 2 + 0.2,
      total_tokens: inputTokens + outputTokens,
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      request_count: Math.floor(Math.random() * 100 + 30),
      image_count: Math.floor(Math.random() * 8),
      audio_seconds: Math.floor(Math.random() * 100),
      character_count: Math.floor(Math.random() * 3000),
    };
  })
).flat();

const mockUsageByPricingSource = [
  {
    pricing_source: "catalog",
    total_cost: 20.1,
    total_tokens: 500000,
    input_tokens: 330000,
    output_tokens: 170000,
    request_count: 1500,
    image_count: 60,
    audio_seconds: 1800,
    character_count: 30000,
  },
  {
    pricing_source: "provider",
    total_cost: 12.4,
    total_tokens: 400000,
    input_tokens: 260000,
    output_tokens: 140000,
    request_count: 1000,
    image_count: 40,
    audio_seconds: 1200,
    character_count: 25000,
  },
  {
    pricing_source: "provider_config",
    total_cost: 6.8,
    total_tokens: 200000,
    input_tokens: 130000,
    output_tokens: 70000,
    request_count: 600,
    image_count: 30,
    audio_seconds: 800,
    character_count: 15000,
  },
  {
    pricing_source: "none",
    total_cost: 3.27,
    total_tokens: 150000,
    input_tokens: 100000,
    output_tokens: 50000,
    request_count: 321,
    image_count: 26,
    audio_seconds: 520,
    character_count: 15000,
  },
];

const PRICING_SOURCES = ["catalog", "provider", "provider_config", "none"];

const mockUsageByDatePricingSource = Array.from({ length: 30 }, (_, i) =>
  PRICING_SOURCES.map((pricing_source) => {
    const inputTokens = Math.floor(Math.random() * 6000 + 1000);
    const outputTokens = Math.floor(Math.random() * 4000 + 500);
    return {
      date: `2026-01-${String(i + 1).padStart(2, "0")}`,
      pricing_source,
      total_cost: Math.random() * 1.2 + 0.05,
      total_tokens: inputTokens + outputTokens,
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      request_count: Math.floor(Math.random() * 40 + 5),
      image_count: Math.floor(Math.random() * 4),
      audio_seconds: Math.floor(Math.random() * 50),
      character_count: Math.floor(Math.random() * 1500),
    };
  })
).flat();

const mockUsageByUser = [
  {
    user_id: "usr-001",
    user_name: "Alice Johnson",
    user_email: "alice@acme-corp.com",
    total_cost: 18.5,
    total_tokens: 500000,
    input_tokens: 330000,
    output_tokens: 170000,
    request_count: 1400,
    image_count: 70,
    audio_seconds: 1600,
    character_count: 30000,
  },
  {
    user_id: "usr-002",
    user_name: "Bob Martinez",
    user_email: "bob@acme-corp.com",
    total_cost: 14.2,
    total_tokens: 420000,
    input_tokens: 280000,
    output_tokens: 140000,
    request_count: 1100,
    image_count: 50,
    audio_seconds: 1400,
    character_count: 28000,
  },
  {
    user_id: "usr-003",
    user_name: null,
    user_email: "charlie@acme-corp.com",
    total_cost: 9.87,
    total_tokens: 330000,
    input_tokens: 210000,
    output_tokens: 120000,
    request_count: 921,
    image_count: 36,
    audio_seconds: 1320,
    character_count: 27000,
  },
];

const mockUsageByDateUser = Array.from({ length: 30 }, (_, i) =>
  mockUsageByUser.map((u) => {
    const inputTokens = Math.floor(Math.random() * 5000 + 1000);
    const outputTokens = Math.floor(Math.random() * 3000 + 500);
    return {
      date: `2026-01-${String(i + 1).padStart(2, "0")}`,
      user_id: u.user_id,
      user_name: u.user_name,
      user_email: u.user_email,
      total_cost: Math.random() * 1.5 + 0.1,
      total_tokens: inputTokens + outputTokens,
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      request_count: Math.floor(Math.random() * 40 + 5),
      image_count: Math.floor(Math.random() * 3),
      audio_seconds: Math.floor(Math.random() * 40),
      character_count: Math.floor(Math.random() * 1000),
    };
  })
).flat();

const mockUsageByProject = [
  {
    project_id: "proj-001",
    project_name: "Production API",
    total_cost: 28.3,
    total_tokens: 750000,
    input_tokens: 490000,
    output_tokens: 260000,
    request_count: 2200,
    image_count: 100,
    audio_seconds: 2800,
    character_count: 55000,
  },
  {
    project_id: "proj-002",
    project_name: "Staging Environment",
    total_cost: 10.1,
    total_tokens: 350000,
    input_tokens: 230000,
    output_tokens: 120000,
    request_count: 850,
    image_count: 40,
    audio_seconds: 1100,
    character_count: 20000,
  },
  {
    project_id: "proj-003",
    project_name: "Research Playground",
    total_cost: 4.17,
    total_tokens: 150000,
    input_tokens: 100000,
    output_tokens: 50000,
    request_count: 371,
    image_count: 16,
    audio_seconds: 420,
    character_count: 10000,
  },
];

const mockUsageByDateProject = Array.from({ length: 30 }, (_, i) =>
  mockUsageByProject.map((p) => {
    const inputTokens = Math.floor(Math.random() * 5000 + 1000);
    const outputTokens = Math.floor(Math.random() * 3000 + 500);
    return {
      date: `2026-01-${String(i + 1).padStart(2, "0")}`,
      project_id: p.project_id,
      project_name: p.project_name,
      total_cost: Math.random() * 1.5 + 0.1,
      total_tokens: inputTokens + outputTokens,
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      request_count: Math.floor(Math.random() * 40 + 5),
      image_count: Math.floor(Math.random() * 3),
      audio_seconds: Math.floor(Math.random() * 40),
      character_count: Math.floor(Math.random() * 1000),
    };
  })
).flat();

const mockUsageByTeam = [
  {
    team_id: "team-abc",
    team_name: "Engineering",
    total_cost: 30.4,
    total_tokens: 800000,
    input_tokens: 520000,
    output_tokens: 280000,
    request_count: 2400,
    image_count: 110,
    audio_seconds: 3000,
    character_count: 60000,
  },
  {
    team_id: "team-def",
    team_name: "Data Science",
    total_cost: 12.17,
    total_tokens: 450000,
    input_tokens: 300000,
    output_tokens: 150000,
    request_count: 1021,
    image_count: 46,
    audio_seconds: 1320,
    character_count: 25000,
  },
];

const mockUsageByDateTeam = Array.from({ length: 30 }, (_, i) =>
  mockUsageByTeam.map((t) => {
    const inputTokens = Math.floor(Math.random() * 5000 + 1000);
    const outputTokens = Math.floor(Math.random() * 3000 + 500);
    return {
      date: `2026-01-${String(i + 1).padStart(2, "0")}`,
      team_id: t.team_id,
      team_name: t.team_name,
      total_cost: Math.random() * 1.5 + 0.1,
      total_tokens: inputTokens + outputTokens,
      input_tokens: inputTokens,
      output_tokens: outputTokens,
      request_count: Math.floor(Math.random() * 40 + 5),
      image_count: Math.floor(Math.random() * 3),
      audio_seconds: Math.floor(Math.random() * 40),
      character_count: Math.floor(Math.random() * 1000),
    };
  })
).flat();

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
  http.get("*/admin/v1/organizations/:slug/usage", () => HttpResponse.json(mockUsageSummary)),
  http.get("*/admin/v1/organizations/:slug/usage/by-date", () =>
    HttpResponse.json(mockUsageByDate)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-model", () =>
    HttpResponse.json(mockUsageByModel)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-provider", () =>
    HttpResponse.json(mockUsageByProvider)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-date-model", () =>
    HttpResponse.json(mockUsageByDateModel)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-date-provider", () =>
    HttpResponse.json(mockUsageByDateProvider)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-pricing-source", () =>
    HttpResponse.json(mockUsageByPricingSource)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-date-pricing-source", () =>
    HttpResponse.json(mockUsageByDatePricingSource)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-user", () =>
    HttpResponse.json(mockUsageByUser)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-date-user", () =>
    HttpResponse.json(mockUsageByDateUser)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-project", () =>
    HttpResponse.json(mockUsageByProject)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-date-project", () =>
    HttpResponse.json(mockUsageByDateProject)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-team", () =>
    HttpResponse.json(mockUsageByTeam)
  ),
  http.get("*/admin/v1/organizations/:slug/usage/by-date-team", () =>
    HttpResponse.json(mockUsageByDateTeam)
  ),
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
            total_cost: 0,
            total_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            request_count: 0,
            image_count: 0,
            audio_seconds: 0,
            character_count: 0,
          });
        }),
        http.get("*/admin/v1/organizations/:slug/usage/*", () => {
          return HttpResponse.json([]);
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
