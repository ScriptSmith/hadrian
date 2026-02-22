import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import UsagePage from "./UsagePage";
import type {
  Organization,
  OrganizationListResponse,
  Team,
  TeamListResponse,
  Project,
  ProjectListResponse,
  User,
  UserListResponse,
  ApiKey,
  ApiKeyListResponse,
  UsageSummaryResponse,
  DailySpendResponse,
  ModelSpendResponse,
  ProviderSpendResponse,
  PricingSourceSpendResponse,
  DailyModelSpendResponse,
  DailyProviderSpendResponse,
  DailyPricingSourceSpendResponse,
} from "@/api/generated/types.gen";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

// --- Mock data ---

const mockOrgs: Organization[] = [
  {
    id: "org-123",
    slug: "acme-corp",
    name: "Acme Corporation",
    created_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "org-456",
    slug: "stark-industries",
    name: "Stark Industries",
    created_at: "2024-02-01T00:00:00Z",
  },
];

const mockOrgsResponse: OrganizationListResponse = {
  data: mockOrgs,
  pagination: { limit: 100, has_more: false },
};

const mockTeams: Team[] = [
  {
    id: "team-1",
    org_id: "org-123",
    slug: "engineering",
    name: "Engineering",
    created_at: "2024-01-10T00:00:00Z",
    updated_at: "2024-01-10T00:00:00Z",
  },
  {
    id: "team-2",
    org_id: "org-123",
    slug: "data-science",
    name: "Data Science",
    created_at: "2024-02-15T00:00:00Z",
    updated_at: "2024-02-15T00:00:00Z",
  },
];

const mockTeamsResponse: TeamListResponse = {
  data: mockTeams,
  pagination: { limit: 100, has_more: false },
};

const mockProjects: Project[] = [
  {
    id: "proj-1",
    org_id: "org-123",
    slug: "production-api",
    name: "Production API",
    created_at: "2024-01-15T00:00:00Z",
    updated_at: "2024-01-15T00:00:00Z",
  },
  {
    id: "proj-2",
    org_id: "org-123",
    slug: "staging-api",
    name: "Staging API",
    team_id: "team-1",
    created_at: "2024-03-01T00:00:00Z",
    updated_at: "2024-03-01T00:00:00Z",
  },
];

const mockProjectsResponse: ProjectListResponse = {
  data: mockProjects,
  pagination: { limit: 100, has_more: false },
};

const mockUsers: User[] = [
  {
    id: "user-1",
    external_id: "alice@acme.com",
    name: "Alice Johnson",
    email: "alice@acme.com",
    created_at: "2024-01-01T00:00:00Z",
    updated_at: "2024-06-01T00:00:00Z",
  },
  {
    id: "user-2",
    external_id: "bob@acme.com",
    name: "Bob Smith",
    email: "bob@acme.com",
    created_at: "2024-02-01T00:00:00Z",
    updated_at: "2024-05-15T00:00:00Z",
  },
];

const mockUsersResponse: UserListResponse = {
  data: mockUsers,
  pagination: { limit: 100, has_more: false },
};

const mockApiKeys: ApiKey[] = [
  {
    id: "key-1",
    name: "Production Key",
    key_prefix: "sk-prod",
    owner: { type: "organization", org_id: "org-123" },
    budget_limit_cents: 50000,
    budget_period: "monthly",
    created_at: "2024-01-01T00:00:00Z",
  },
  {
    id: "key-2",
    name: "Dev Key",
    key_prefix: "sk-dev",
    owner: { type: "user", user_id: "user-1" },
    created_at: "2024-03-01T00:00:00Z",
  },
  {
    id: "key-3",
    name: "Revoked Key",
    key_prefix: "sk-old",
    owner: { type: "organization", org_id: "org-123" },
    revoked_at: "2024-06-01T00:00:00Z",
    created_at: "2024-02-01T00:00:00Z",
  },
];

const mockApiKeysResponse: ApiKeyListResponse = {
  data: mockApiKeys,
  pagination: { limit: 100, has_more: false },
};

// --- Usage mock data ---

function generateDates(days: number): string[] {
  const dates: string[] = [];
  const now = new Date();
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(now);
    d.setDate(d.getDate() - i);
    dates.push(d.toISOString().split("T")[0]);
  }
  return dates;
}

const dates = generateDates(30);

const mockSummary: UsageSummaryResponse = {
  total_cost: 1247.83,
  total_tokens: 12_450_000,
  input_tokens: 8_300_000,
  output_tokens: 4_150_000,
  request_count: 15_432,
  first_request_at: "2024-01-15T08:30:00Z",
  last_request_at: "2024-07-14T22:15:00Z",
};

const mockDailySpend: DailySpendResponse[] = dates.map((date, i) => ({
  date,
  total_cost: 30 + Math.sin(i * 0.5) * 15 + Math.random() * 10,
  total_tokens: 300_000 + Math.floor(Math.random() * 200_000),
  input_tokens: 200_000 + Math.floor(Math.random() * 100_000),
  output_tokens: 100_000 + Math.floor(Math.random() * 100_000),
  request_count: 400 + Math.floor(Math.random() * 200),
}));

const mockModelSpend: ModelSpendResponse[] = [
  {
    model: "gpt-4o",
    total_cost: 523.45,
    total_tokens: 5_200_000,
    input_tokens: 3_500_000,
    output_tokens: 1_700_000,
    request_count: 6_200,
  },
  {
    model: "claude-3.5-sonnet",
    total_cost: 412.18,
    total_tokens: 4_100_000,
    input_tokens: 2_800_000,
    output_tokens: 1_300_000,
    request_count: 5_100,
  },
  {
    model: "gpt-4o-mini",
    total_cost: 187.92,
    total_tokens: 1_900_000,
    input_tokens: 1_200_000,
    output_tokens: 700_000,
    request_count: 2_800,
  },
  {
    model: "claude-3-haiku",
    total_cost: 124.28,
    total_tokens: 1_250_000,
    input_tokens: 800_000,
    output_tokens: 450_000,
    request_count: 1_332,
  },
];

const mockProviderSpend: ProviderSpendResponse[] = [
  {
    provider: "openai",
    total_cost: 711.37,
    total_tokens: 7_100_000,
    input_tokens: 4_700_000,
    output_tokens: 2_400_000,
    request_count: 9_000,
  },
  {
    provider: "anthropic",
    total_cost: 536.46,
    total_tokens: 5_350_000,
    input_tokens: 3_600_000,
    output_tokens: 1_750_000,
    request_count: 6_432,
  },
];

const mockPricingSourceSpend: PricingSourceSpendResponse[] = [
  {
    pricing_source: "catalog",
    total_cost: 987.6,
    total_tokens: 9_800_000,
    input_tokens: 6_500_000,
    output_tokens: 3_300_000,
    request_count: 12_100,
  },
  {
    pricing_source: "provider_config",
    total_cost: 260.23,
    total_tokens: 2_650_000,
    input_tokens: 1_800_000,
    output_tokens: 850_000,
    request_count: 3_332,
  },
];

const models = ["gpt-4o", "claude-3.5-sonnet", "gpt-4o-mini", "claude-3-haiku"];
const providers = ["openai", "anthropic"];
const pricingSources = ["catalog", "provider_config"];

const mockDailyModelSpend: DailyModelSpendResponse[] = dates.flatMap((date) =>
  models.map((model) => ({
    date,
    model,
    total_cost: 5 + Math.random() * 15,
    total_tokens: 50_000 + Math.floor(Math.random() * 50_000),
    input_tokens: 30_000 + Math.floor(Math.random() * 30_000),
    output_tokens: 20_000 + Math.floor(Math.random() * 20_000),
    request_count: 50 + Math.floor(Math.random() * 100),
  }))
);

const mockDailyProviderSpend: DailyProviderSpendResponse[] = dates.flatMap((date) =>
  providers.map((provider) => ({
    date,
    provider,
    total_cost: 10 + Math.random() * 30,
    total_tokens: 100_000 + Math.floor(Math.random() * 100_000),
    input_tokens: 60_000 + Math.floor(Math.random() * 60_000),
    output_tokens: 40_000 + Math.floor(Math.random() * 40_000),
    request_count: 100 + Math.floor(Math.random() * 200),
  }))
);

const mockDailyPricingSourceSpend: DailyPricingSourceSpendResponse[] = dates.flatMap((date) =>
  pricingSources.map((pricing_source) => ({
    date,
    pricing_source,
    total_cost: 8 + Math.random() * 25,
    total_tokens: 80_000 + Math.floor(Math.random() * 80_000),
    input_tokens: 50_000 + Math.floor(Math.random() * 50_000),
    output_tokens: 30_000 + Math.floor(Math.random() * 30_000),
    request_count: 80 + Math.floor(Math.random() * 150),
  }))
);

const emptySummary: UsageSummaryResponse = {
  total_cost: 0,
  total_tokens: 0,
  input_tokens: 0,
  output_tokens: 0,
  request_count: 0,
  first_request_at: null,
  last_request_at: null,
};

// --- MSW handler helpers ---

/** Handlers for all org-scoped usage endpoints with the given data */
function orgUsageHandlers(
  slug: string,
  options: {
    summary: UsageSummaryResponse;
    daily: DailySpendResponse[];
    byModel: ModelSpendResponse[];
    byProvider: ProviderSpendResponse[];
    byPricingSource: PricingSourceSpendResponse[];
    byDateModel: DailyModelSpendResponse[];
    byDateProvider: DailyProviderSpendResponse[];
    byDatePricingSource: DailyPricingSourceSpendResponse[];
  }
) {
  return [
    http.get(`*/admin/v1/organizations/${slug}/usage`, () => HttpResponse.json(options.summary)),
    http.get(`*/admin/v1/organizations/${slug}/usage/by-date`, () =>
      HttpResponse.json(options.daily)
    ),
    http.get(`*/admin/v1/organizations/${slug}/usage/by-model`, () =>
      HttpResponse.json(options.byModel)
    ),
    http.get(`*/admin/v1/organizations/${slug}/usage/by-provider`, () =>
      HttpResponse.json(options.byProvider)
    ),
    http.get(`*/admin/v1/organizations/${slug}/usage/by-pricing-source`, () =>
      HttpResponse.json(options.byPricingSource)
    ),
    http.get(`*/admin/v1/organizations/${slug}/usage/by-date-model`, () =>
      HttpResponse.json(options.byDateModel)
    ),
    http.get(`*/admin/v1/organizations/${slug}/usage/by-date-provider`, () =>
      HttpResponse.json(options.byDateProvider)
    ),
    http.get(`*/admin/v1/organizations/${slug}/usage/by-date-pricing-source`, () =>
      HttpResponse.json(options.byDatePricingSource)
    ),
  ];
}

/** Common filter-list handlers (orgs, teams, projects, users, api keys) */
const filterHandlers = [
  http.get("*/admin/v1/organizations", () => HttpResponse.json(mockOrgsResponse)),
  http.get("*/admin/v1/organizations/acme-corp/teams", () => HttpResponse.json(mockTeamsResponse)),
  http.get("*/admin/v1/organizations/acme-corp/projects", () =>
    HttpResponse.json(mockProjectsResponse)
  ),
  http.get("*/admin/v1/users", () => HttpResponse.json(mockUsersResponse)),
  http.get("*/admin/v1/organizations/acme-corp/api-keys", () =>
    HttpResponse.json(mockApiKeysResponse)
  ),
  // Second org returns empty lists
  http.get("*/admin/v1/organizations/stark-industries/teams", () =>
    HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } })
  ),
  http.get("*/admin/v1/organizations/stark-industries/projects", () =>
    HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } })
  ),
  http.get("*/admin/v1/organizations/stark-industries/api-keys", () =>
    HttpResponse.json({ data: [], pagination: { limit: 100, has_more: false } })
  ),
];

const fullUsageData = {
  summary: mockSummary,
  daily: mockDailySpend,
  byModel: mockModelSpend,
  byProvider: mockProviderSpend,
  byPricingSource: mockPricingSourceSpend,
  byDateModel: mockDailyModelSpend,
  byDateProvider: mockDailyProviderSpend,
  byDatePricingSource: mockDailyPricingSourceSpend,
};

const emptyUsageData = {
  summary: emptySummary,
  daily: [] as DailySpendResponse[],
  byModel: [] as ModelSpendResponse[],
  byProvider: [] as ProviderSpendResponse[],
  byPricingSource: [] as PricingSourceSpendResponse[],
  byDateModel: [] as DailyModelSpendResponse[],
  byDateProvider: [] as DailyProviderSpendResponse[],
  byDatePricingSource: [] as DailyPricingSourceSpendResponse[],
};

// --- Story meta ---

const meta: Meta<typeof UsagePage> = {
  title: "Admin/UsagePage",
  component: UsagePage,
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
        <MemoryRouter initialEntries={["/admin/usage"]}>
          <Routes>
            <Route path="/admin/usage" element={<Story />} />
          </Routes>
        </MemoryRouter>
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
        ...filterHandlers,
        ...orgUsageHandlers("acme-corp", fullUsageData),
        ...orgUsageHandlers("stark-industries", emptyUsageData),
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
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        ...filterHandlers,
        ...orgUsageHandlers("acme-corp", emptyUsageData),
        ...orgUsageHandlers("stark-industries", emptyUsageData),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        ...filterHandlers,
        http.get("*/admin/v1/organizations/acme-corp/usage", () =>
          HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          )
        ),
        http.get("*/admin/v1/organizations/acme-corp/usage/*", () =>
          HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          )
        ),
      ],
    },
  },
};
