import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import UsageLogsTable from "./UsageLogsTable";
import type { UsageLogResponse, UsageLogListResponse } from "@/api/generated/types.gen";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const mockLogs: UsageLogResponse[] = [
  {
    id: "rec-001",
    recorded_at: "2024-06-15T14:30:00Z",
    request_id: "req-abc123def456",
    model: "gpt-4o",
    provider: "openai",
    provider_source: "static",
    input_tokens: 1250,
    output_tokens: 340,
    cached_tokens: 200,
    reasoning_tokens: 0,
    cost: 0.0245,
    streamed: true,
    finish_reason: "stop",
    latency_ms: 1520,
    cancelled: false,
    status_code: 200,
    pricing_source: "catalog",
    user_id: "usr-001",
    api_key_id: "key-001",
    org_id: "org-123",
    project_id: "proj-001",
    team_id: null,
    service_account_id: null,
    http_referer: null,
    image_count: 2,
    audio_seconds: null,
    character_count: null,
  },
  {
    id: "rec-002",
    recorded_at: "2024-06-15T14:28:00Z",
    request_id: "req-def789ghi012",
    model: "claude-sonnet-4-20250514",
    provider: "anthropic",
    provider_source: "dynamic",
    input_tokens: 3400,
    output_tokens: 1200,
    cached_tokens: 0,
    reasoning_tokens: 800,
    cost: 0.0892,
    streamed: false,
    finish_reason: "stop",
    latency_ms: 4200,
    cancelled: false,
    status_code: 200,
    pricing_source: "provider",
    user_id: "usr-002",
    api_key_id: "key-002",
    org_id: "org-123",
    project_id: null,
    team_id: "team-001",
    service_account_id: null,
    http_referer: null,
    image_count: null,
    audio_seconds: 45,
    character_count: 3200,
  },
  {
    id: "rec-003",
    recorded_at: "2024-06-15T14:25:00Z",
    request_id: "req-jkl345mno678",
    model: "gpt-4o-mini",
    provider: "openai",
    provider_source: "static",
    input_tokens: 500,
    output_tokens: 150,
    cached_tokens: 0,
    reasoning_tokens: 0,
    cost: 0.0012,
    streamed: true,
    finish_reason: "length",
    latency_ms: 890,
    cancelled: false,
    status_code: 200,
    pricing_source: "catalog",
    user_id: null,
    api_key_id: "key-003",
    org_id: "org-456",
    project_id: null,
    team_id: null,
    service_account_id: "sa-001",
    http_referer: null,
    image_count: null,
    audio_seconds: null,
    character_count: null,
  },
  {
    id: "rec-004",
    recorded_at: "2024-06-15T14:20:00Z",
    request_id: "req-pqr901stu234",
    model: "gpt-4o",
    provider: "openai",
    provider_source: "static",
    input_tokens: 2000,
    output_tokens: 0,
    cached_tokens: 0,
    reasoning_tokens: 0,
    cost: 0,
    streamed: false,
    finish_reason: "error",
    latency_ms: 200,
    cancelled: false,
    status_code: 429,
    pricing_source: "none",
    user_id: "usr-001",
    api_key_id: "key-001",
    org_id: "org-123",
    project_id: "proj-001",
    team_id: null,
    service_account_id: null,
    http_referer: null,
    image_count: null,
    audio_seconds: null,
    character_count: null,
  },
];

const mockResponse: UsageLogListResponse = {
  data: mockLogs,
  pagination: {
    limit: 50,
    has_more: false,
  },
};

const mockUsersResponse = {
  data: [
    {
      id: "usr-001",
      name: "Alice Johnson",
      email: "alice@example.com",
      external_id: "alice",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
    {
      id: "usr-002",
      name: "Bob Martinez",
      email: "bob@example.com",
      external_id: "bob",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
  ],
  pagination: { limit: 100, has_more: false },
};

const emptyResponse: UsageLogListResponse = {
  data: [],
  pagination: {
    limit: 50,
    has_more: false,
  },
};

const meta: Meta<typeof UsageLogsTable> = {
  title: "Admin/UsageLogsTable",
  component: UsageLogsTable,
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
            <Route
              path="/admin/usage"
              element={
                <div className="p-6">
                  <Story />
                </div>
              }
            />
          </Routes>
        </MemoryRouter>
      </QueryClientProvider>
    ),
  ],
  args: {
    scope: { type: "global" },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/usage/logs", () => {
          return HttpResponse.json(mockResponse);
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(mockUsersResponse);
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/usage/logs", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockResponse);
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(mockUsersResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/usage/logs", () => {
          return HttpResponse.json(emptyResponse);
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(mockUsersResponse);
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/usage/logs", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(mockUsersResponse);
        }),
      ],
    },
  },
};

export const MeScope: Story = {
  args: {
    scope: { type: "me" },
  },
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/me/usage/logs", () => {
          return HttpResponse.json(mockResponse);
        }),
      ],
    },
  },
};

export const WithPagination: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/usage/logs", () => {
          const models = ["gpt-4o", "claude-sonnet-4-20250514", "gpt-4o-mini", "llama-3.1-70b"];
          const providers = ["openai", "anthropic", "groq"];
          const finishReasons = ["stop", "length", "error", "content_filter"];

          const manyLogs: UsageLogResponse[] = Array.from({ length: 50 }, (_, i) => ({
            id: `rec-${i + 1}`,
            recorded_at: new Date(2024, 5, 15, 23, 59 - i).toISOString(),
            request_id: `req-${i + 1}-abcdef123456`,
            model: models[i % models.length],
            provider: providers[i % providers.length],
            provider_source: i % 2 === 0 ? "static" : "dynamic",
            input_tokens: 500 + i * 100,
            output_tokens: 100 + i * 50,
            cached_tokens: i % 3 === 0 ? 200 : 0,
            reasoning_tokens: i % 4 === 0 ? 500 : 0,
            cost: (0.001 + i * 0.005) * (i % 3 === 0 ? 0 : 1),
            streamed: i % 2 === 0,
            finish_reason: finishReasons[i % finishReasons.length],
            latency_ms: 200 + i * 100,
            cancelled: false,
            status_code: i % 10 === 9 ? 429 : 200,
            pricing_source: "catalog",
            user_id: `usr-${(i % 5) + 1}`,
            api_key_id: `key-${(i % 3) + 1}`,
            org_id: "org-123",
            project_id: i % 2 === 0 ? "proj-001" : null,
            team_id: null,
            service_account_id: null,
            http_referer: null,
            image_count: null,
            audio_seconds: null,
            character_count: null,
          }));

          return HttpResponse.json({
            data: manyLogs,
            pagination: {
              limit: 50,
              has_more: true,
              next_cursor: "bW9ja19jdXJzb3I=",
            },
          });
        }),
        http.get("*/admin/v1/users", () => {
          return HttpResponse.json(mockUsersResponse);
        }),
      ],
    },
  },
};
