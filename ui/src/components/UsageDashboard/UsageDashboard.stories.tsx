import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";

import UsageDashboard from "./UsageDashboard";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const mockSummary = {
  total_cost: 42.57,
  total_tokens: 1_250_000,
  input_tokens: 820_000,
  output_tokens: 430_000,
  request_count: 3_421,
  first_request_at: "2025-12-01T08:00:00Z",
  last_request_at: "2025-12-30T23:59:00Z",
  image_count: 156,
  audio_seconds: 4_320,
  character_count: 85_000,
};

const mockByDate = Array.from({ length: 30 }, (_, i) => {
  const inputTokens = Math.floor(Math.random() * 30000 + 8000);
  const outputTokens = Math.floor(Math.random() * 20000 + 5000);
  return {
    date: `2025-12-${String(i + 1).padStart(2, "0")}`,
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

const mockByModel = [
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

const mockByProvider = [
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

const MODELS = ["gpt-4o", "claude-opus-4-6", "gpt-4o-mini", "claude-haiku-4-5"];
const PROVIDERS = ["openai", "anthropic"];
const PRICING_SOURCES = ["catalog", "provider", "provider_config", "pricing_config", "none"];

const mockByDateModel = Array.from({ length: 30 }, (_, i) =>
  MODELS.map((model) => {
    const inputTokens = Math.floor(Math.random() * 8000 + 2000);
    const outputTokens = Math.floor(Math.random() * 5000 + 1000);
    return {
      date: `2025-12-${String(i + 1).padStart(2, "0")}`,
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

const mockByDateProvider = Array.from({ length: 30 }, (_, i) =>
  PROVIDERS.map((provider) => {
    const inputTokens = Math.floor(Math.random() * 15000 + 5000);
    const outputTokens = Math.floor(Math.random() * 10000 + 3000);
    return {
      date: `2025-12-${String(i + 1).padStart(2, "0")}`,
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

const mockByPricingSource = [
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
    pricing_source: "pricing_config",
    total_cost: 2.5,
    total_tokens: 100000,
    input_tokens: 65000,
    output_tokens: 35000,
    request_count: 250,
    image_count: 16,
    audio_seconds: 320,
    character_count: 10000,
  },
  {
    pricing_source: "none",
    total_cost: 0.77,
    total_tokens: 50000,
    input_tokens: 35000,
    output_tokens: 15000,
    request_count: 71,
    image_count: 10,
    audio_seconds: 200,
    character_count: 5000,
  },
];

const mockByDatePricingSource = Array.from({ length: 30 }, (_, i) =>
  PRICING_SOURCES.map((pricing_source) => {
    const inputTokens = Math.floor(Math.random() * 6000 + 1000);
    const outputTokens = Math.floor(Math.random() * 4000 + 500);
    return {
      date: `2025-12-${String(i + 1).padStart(2, "0")}`,
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

const USERS = [
  { user_id: "u1", user_name: "Alice Smith", user_email: "alice@acme.com" },
  { user_id: "u2", user_name: "Bob Jones", user_email: "bob@acme.com" },
  { user_id: "u3", user_name: null, user_email: "charlie@acme.com" },
];
const PROJECTS = [
  { project_id: "p1", project_name: "production-api" },
  { project_id: "p2", project_name: "staging-api" },
];
const TEAMS = [
  { team_id: "t1", team_name: "engineering" },
  { team_id: "t2", team_name: "data-science" },
];
const ORGS = [
  { org_id: "o1", org_name: "Acme Corp" },
  { org_id: "o2", org_name: "Beta Inc" },
];

function makeEntitySpend<T>(entities: T[]) {
  return entities.map((e, i) => ({
    ...e,
    total_cost: 15 - i * 4,
    total_tokens: 400000 - i * 100000,
    input_tokens: 260000 - i * 60000,
    output_tokens: 140000 - i * 40000,
    request_count: 1200 - i * 300,
    image_count: 40 - i * 10,
    audio_seconds: 1000 - i * 200,
    character_count: 20000 - i * 5000,
  }));
}

function makeDailyEntitySpend<T>(entities: T[], keyFn: (e: T) => Record<string, unknown>) {
  return Array.from({ length: 30 }, (_, i) =>
    entities.map((e) => {
      const inputTokens = Math.floor(Math.random() * 5000 + 1000);
      const outputTokens = Math.floor(Math.random() * 3000 + 500);
      return {
        date: `2025-12-${String(i + 1).padStart(2, "0")}`,
        ...keyFn(e),
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
}

const mockByUser = makeEntitySpend(USERS);
const mockByProject = makeEntitySpend(PROJECTS);
const mockByTeam = makeEntitySpend(TEAMS);
const mockByOrg = makeEntitySpend(ORGS);

const mockByDateUser = makeDailyEntitySpend(USERS, (u) => ({
  user_id: u.user_id,
  user_name: u.user_name,
  user_email: u.user_email,
}));
const mockByDateProject = makeDailyEntitySpend(PROJECTS, (p) => ({
  project_id: p.project_id,
  project_name: p.project_name,
}));
const mockByDateTeam = makeDailyEntitySpend(TEAMS, (t) => ({
  team_id: t.team_id,
  team_name: t.team_name,
}));
const mockByDateOrg = makeDailyEntitySpend(ORGS, (o) => ({
  org_id: o.org_id,
  org_name: o.org_name,
}));

// Wildcard handlers match all scope-specific URL patterns
const handlers = [
  // Summary
  http.get("*/admin/v1/api-keys/*/usage", () => HttpResponse.json(mockSummary)),
  http.get("*/admin/v1/organizations/*/usage", () => HttpResponse.json(mockSummary)),
  http.get("*/admin/v1/organizations/*/projects/*/usage", () => HttpResponse.json(mockSummary)),
  http.get("*/admin/v1/organizations/*/teams/*/usage", () => HttpResponse.json(mockSummary)),
  http.get("*/admin/v1/users/*/usage", () => HttpResponse.json(mockSummary)),
  http.get("*/admin/v1/me/usage", () => HttpResponse.json(mockSummary)),
  // By Date
  http.get("*/admin/v1/api-keys/*/usage/by-date", () => HttpResponse.json(mockByDate)),
  http.get("*/admin/v1/organizations/*/usage/by-date", () => HttpResponse.json(mockByDate)),
  http.get("*/admin/v1/organizations/*/projects/*/usage/by-date", () =>
    HttpResponse.json(mockByDate)
  ),
  http.get("*/admin/v1/organizations/*/teams/*/usage/by-date", () => HttpResponse.json(mockByDate)),
  http.get("*/admin/v1/users/*/usage/by-date", () => HttpResponse.json(mockByDate)),
  http.get("*/admin/v1/me/usage/by-date", () => HttpResponse.json(mockByDate)),
  // By Model
  http.get("*/admin/v1/api-keys/*/usage/by-model", () => HttpResponse.json(mockByModel)),
  http.get("*/admin/v1/organizations/*/usage/by-model", () => HttpResponse.json(mockByModel)),
  http.get("*/admin/v1/organizations/*/projects/*/usage/by-model", () =>
    HttpResponse.json(mockByModel)
  ),
  http.get("*/admin/v1/organizations/*/teams/*/usage/by-model", () =>
    HttpResponse.json(mockByModel)
  ),
  http.get("*/admin/v1/users/*/usage/by-model", () => HttpResponse.json(mockByModel)),
  http.get("*/admin/v1/me/usage/by-model", () => HttpResponse.json(mockByModel)),
  // By Provider
  http.get("*/admin/v1/api-keys/*/usage/by-provider", () => HttpResponse.json(mockByProvider)),
  http.get("*/admin/v1/organizations/*/usage/by-provider", () => HttpResponse.json(mockByProvider)),
  http.get("*/admin/v1/organizations/*/projects/*/usage/by-provider", () =>
    HttpResponse.json(mockByProvider)
  ),
  http.get("*/admin/v1/organizations/*/teams/*/usage/by-provider", () =>
    HttpResponse.json(mockByProvider)
  ),
  http.get("*/admin/v1/users/*/usage/by-provider", () => HttpResponse.json(mockByProvider)),
  http.get("*/admin/v1/me/usage/by-provider", () => HttpResponse.json(mockByProvider)),
  // By Date+Model
  http.get("*/admin/v1/api-keys/*/usage/by-date-model", () => HttpResponse.json(mockByDateModel)),
  http.get("*/admin/v1/organizations/*/usage/by-date-model", () =>
    HttpResponse.json(mockByDateModel)
  ),
  http.get("*/admin/v1/organizations/*/projects/*/usage/by-date-model", () =>
    HttpResponse.json(mockByDateModel)
  ),
  http.get("*/admin/v1/organizations/*/teams/*/usage/by-date-model", () =>
    HttpResponse.json(mockByDateModel)
  ),
  http.get("*/admin/v1/users/*/usage/by-date-model", () => HttpResponse.json(mockByDateModel)),
  http.get("*/admin/v1/me/usage/by-date-model", () => HttpResponse.json(mockByDateModel)),
  // By Date+Provider
  http.get("*/admin/v1/api-keys/*/usage/by-date-provider", () =>
    HttpResponse.json(mockByDateProvider)
  ),
  http.get("*/admin/v1/organizations/*/usage/by-date-provider", () =>
    HttpResponse.json(mockByDateProvider)
  ),
  http.get("*/admin/v1/organizations/*/projects/*/usage/by-date-provider", () =>
    HttpResponse.json(mockByDateProvider)
  ),
  http.get("*/admin/v1/organizations/*/teams/*/usage/by-date-provider", () =>
    HttpResponse.json(mockByDateProvider)
  ),
  http.get("*/admin/v1/users/*/usage/by-date-provider", () =>
    HttpResponse.json(mockByDateProvider)
  ),
  http.get("*/admin/v1/me/usage/by-date-provider", () => HttpResponse.json(mockByDateProvider)),
  // By Pricing Source
  http.get("*/admin/v1/api-keys/*/usage/by-pricing-source", () =>
    HttpResponse.json(mockByPricingSource)
  ),
  http.get("*/admin/v1/organizations/*/usage/by-pricing-source", () =>
    HttpResponse.json(mockByPricingSource)
  ),
  http.get("*/admin/v1/organizations/*/projects/*/usage/by-pricing-source", () =>
    HttpResponse.json(mockByPricingSource)
  ),
  http.get("*/admin/v1/organizations/*/teams/*/usage/by-pricing-source", () =>
    HttpResponse.json(mockByPricingSource)
  ),
  http.get("*/admin/v1/users/*/usage/by-pricing-source", () =>
    HttpResponse.json(mockByPricingSource)
  ),
  http.get("*/admin/v1/me/usage/by-pricing-source", () => HttpResponse.json(mockByPricingSource)),
  // By Date+Pricing Source
  http.get("*/admin/v1/api-keys/*/usage/by-date-pricing-source", () =>
    HttpResponse.json(mockByDatePricingSource)
  ),
  http.get("*/admin/v1/organizations/*/usage/by-date-pricing-source", () =>
    HttpResponse.json(mockByDatePricingSource)
  ),
  http.get("*/admin/v1/organizations/*/projects/*/usage/by-date-pricing-source", () =>
    HttpResponse.json(mockByDatePricingSource)
  ),
  http.get("*/admin/v1/organizations/*/teams/*/usage/by-date-pricing-source", () =>
    HttpResponse.json(mockByDatePricingSource)
  ),
  http.get("*/admin/v1/users/*/usage/by-date-pricing-source", () =>
    HttpResponse.json(mockByDatePricingSource)
  ),
  http.get("*/admin/v1/me/usage/by-date-pricing-source", () =>
    HttpResponse.json(mockByDatePricingSource)
  ),
  // Global scope
  http.get("*/admin/v1/usage", () => HttpResponse.json(mockSummary)),
  http.get("*/admin/v1/usage/by-date", () => HttpResponse.json(mockByDate)),
  http.get("*/admin/v1/usage/by-model", () => HttpResponse.json(mockByModel)),
  http.get("*/admin/v1/usage/by-provider", () => HttpResponse.json(mockByProvider)),
  http.get("*/admin/v1/usage/by-pricing-source", () => HttpResponse.json(mockByPricingSource)),
  http.get("*/admin/v1/usage/by-date-model", () => HttpResponse.json(mockByDateModel)),
  http.get("*/admin/v1/usage/by-date-provider", () => HttpResponse.json(mockByDateProvider)),
  http.get("*/admin/v1/usage/by-date-pricing-source", () =>
    HttpResponse.json(mockByDatePricingSource)
  ),
  // Entity breakdowns (by-user, by-project, by-team, by-org)
  http.get("*/usage/by-user", () => HttpResponse.json(mockByUser)),
  http.get("*/usage/by-date-user", () => HttpResponse.json(mockByDateUser)),
  http.get("*/usage/by-project", () => HttpResponse.json(mockByProject)),
  http.get("*/usage/by-date-project", () => HttpResponse.json(mockByDateProject)),
  http.get("*/usage/by-team", () => HttpResponse.json(mockByTeam)),
  http.get("*/usage/by-date-team", () => HttpResponse.json(mockByDateTeam)),
  http.get("*/usage/by-org", () => HttpResponse.json(mockByOrg)),
  http.get("*/usage/by-date-org", () => HttpResponse.json(mockByDateOrg)),
];

const meta: Meta<typeof UsageDashboard> = {
  title: "Components/UsageDashboard",
  component: UsageDashboard,
  decorators: [
    (Story) => (
      <QueryClientProvider client={queryClient}>
        <div className="p-6">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
  parameters: {
    msw: { handlers },
  },
};

export default meta;
type Story = StoryObj<typeof UsageDashboard>;

export const Organization: Story = {
  args: {
    scope: { type: "organization", slug: "acme-corp" },
  },
};

export const Team: Story = {
  args: {
    scope: { type: "team", orgSlug: "acme-corp", teamSlug: "engineering" },
  },
};

export const Project: Story = {
  args: {
    scope: { type: "project", orgSlug: "acme-corp", projectSlug: "production-api" },
  },
};

export const SelfService: Story = {
  args: {
    scope: { type: "me" },
  },
};

export const Global: Story = {
  args: {
    scope: { type: "global" },
  },
};
