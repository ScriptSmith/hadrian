import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import UsageLogsTable from "./UsageLogsTable";
import { ToastProvider } from "@/components/Toast/Toast";
import type { UsageLogResponse, UsageLogListResponse } from "@/api/generated/types.gen";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const models = [
  { model: "gpt-5.4", provider: "openai" },
  { model: "gpt-5.4-mini", provider: "openai" },
  { model: "o3", provider: "openai" },
  { model: "claude-opus-4-7-20260301", provider: "anthropic" },
  { model: "claude-sonnet-4-6-20260301", provider: "anthropic" },
  { model: "claude-haiku-4-5-20251001", provider: "anthropic" },
  { model: "gemini-3.1-pro", provider: "vertex_ai" },
  { model: "gemini-3.1-flash", provider: "vertex_ai" },
  { model: "llama-4-maverick", provider: "bedrock" },
  { model: "nova-premier", provider: "bedrock" },
] as const;

const finishReasons = [
  "stop",
  "stop",
  "stop",
  "stop",
  "length",
  "error",
  "content_filter",
] as const;

const mockLogs: UsageLogResponse[] = Array.from({ length: 30 }, (_, i) => {
  const m = models[i % models.length];
  const isError = i === 22;
  const isCancelled = i === 17;
  return {
    id: `rec-${String(i + 1).padStart(3, "0")}`,
    recorded_at: new Date(2026, 2, 16, 15, 59 - i * 2, (i * 17) % 60).toISOString(),
    request_id: `req-${crypto.randomUUID?.() ?? `${i}-abcdef123456`}`,
    model: m.model,
    provider: m.provider,
    provider_source: i % 3 === 0 ? "dynamic" : "static",
    input_tokens: 400 + i * 120,
    output_tokens: isError ? 0 : 80 + i * 60,
    cached_tokens: i % 4 === 0 ? 300 + i * 50 : 0,
    reasoning_tokens: ["o3", "claude-opus-4-7-20260301"].includes(m.model) ? 600 + i * 80 : 0,
    cost: isError ? 0 : parseFloat((0.002 + i * 0.008).toFixed(4)),
    streamed: i % 2 === 0,
    finish_reason: isError
      ? "error"
      : isCancelled
        ? "stop"
        : finishReasons[i % finishReasons.length],
    latency_ms: isError ? 180 : 300 + i * 90,
    cancelled: isCancelled,
    status_code: isError ? 429 : 200,
    pricing_source: isError ? "none" : i % 3 === 0 ? "provider" : "catalog",
    user_id: i % 6 === 0 ? null : `usr-${(i % 5) + 1}`,
    api_key_id: `key-${(i % 4) + 1}`,
    org_id: "org-123",
    project_id: i % 3 !== 2 ? `proj-${(i % 3) + 1}` : null,
    team_id: i % 4 === 1 ? `team-${(i % 2) + 1}` : null,
    service_account_id: i % 6 === 0 ? `sa-${(i % 2) + 1}` : null,
    http_referer: null,
    image_count: m.model === "gpt-5.4" && i % 5 === 0 ? 2 : null,
    audio_seconds: m.provider === "openai" && i % 7 === 0 ? 30 + i : null,
    character_count: m.provider === "openai" && i % 7 === 0 ? 2400 + i * 100 : null,
  };
});

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
      id: "usr-1",
      name: "Alice Johnson",
      email: "alice@example.com",
      external_id: "alice",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
    {
      id: "usr-2",
      name: "Bob Martinez",
      email: "bob@example.com",
      external_id: "bob",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
    {
      id: "usr-3",
      name: "Carol Wei",
      email: "carol@example.com",
      external_id: "carol",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
    {
      id: "usr-4",
      name: "David Kim",
      email: "david@example.com",
      external_id: "david",
      created_at: "2024-01-01T00:00:00Z",
      updated_at: "2024-01-01T00:00:00Z",
    },
    {
      id: "usr-5",
      name: "Elena Rossi",
      email: "elena@example.com",
      external_id: "elena",
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
        <ToastProvider>
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
        </ToastProvider>
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
          const pgModels = [
            "gpt-5.4",
            "claude-opus-4-7-20260301",
            "gemini-3.1-pro",
            "o3",
            "claude-sonnet-4-6-20260301",
            "gpt-5.4-mini",
            "llama-4-maverick",
            "gemini-3.1-flash",
          ];
          const pgProviders = [
            "openai",
            "anthropic",
            "vertex_ai",
            "openai",
            "anthropic",
            "openai",
            "bedrock",
            "vertex_ai",
          ];
          const pgFinishReasons = ["stop", "stop", "stop", "length", "error", "content_filter"];

          const manyLogs: UsageLogResponse[] = Array.from({ length: 50 }, (_, i) => ({
            id: `rec-${i + 1}`,
            recorded_at: new Date(2026, 2, 16, 23, 59 - i).toISOString(),
            request_id: `req-${i + 1}-abcdef123456`,
            model: pgModels[i % pgModels.length]!,
            provider: pgProviders[i % pgProviders.length]!,
            provider_source: i % 2 === 0 ? "static" : "dynamic",
            input_tokens: 500 + i * 100,
            output_tokens: 100 + i * 50,
            cached_tokens: i % 3 === 0 ? 200 : 0,
            reasoning_tokens: i % 4 === 0 ? 500 : 0,
            cost: (0.001 + i * 0.005) * (i % 3 === 0 ? 0 : 1),
            streamed: i % 2 === 0,
            finish_reason: pgFinishReasons[i % pgFinishReasons.length]!,
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
