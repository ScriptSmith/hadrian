import type { Meta, StoryObj } from "@storybook/react";
import { http, HttpResponse } from "msw";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import MyProvidersPage from "./MyProvidersPage";
import { ToastProvider } from "@/components/Toast/Toast";
import { ConfirmDialogProvider } from "@/components/ConfirmDialog/ConfirmDialog";

const mockProviders = {
  data: [
    {
      id: "prov-1",
      name: "my-openai",
      provider_type: "open_ai",
      base_url: "https://api.openai.com/v1",
      has_api_key: true,
      config: null,
      models: ["gpt-4o", "gpt-4o-mini", "gpt-3.5-turbo"],
      owner: { type: "user", user_id: "user-1" },
      is_enabled: true,
      created_at: "2024-06-01T00:00:00Z",
      updated_at: "2024-06-01T00:00:00Z",
    },
    {
      id: "prov-2",
      name: "my-anthropic",
      provider_type: "anthropic",
      base_url: "https://api.anthropic.com",
      has_api_key: true,
      config: null,
      models: ["claude-sonnet-4-5-20250929", "claude-haiku-3-5-20241022"],
      owner: { type: "user", user_id: "user-1" },
      is_enabled: true,
      created_at: "2024-07-15T00:00:00Z",
      updated_at: "2024-07-15T00:00:00Z",
    },
    {
      id: "prov-3",
      name: "my-groq",
      provider_type: "open_ai",
      base_url: "https://api.groq.com/openai/v1",
      has_api_key: true,
      config: null,
      models: ["llama-3.1-70b-versatile", "mixtral-8x7b-32768"],
      owner: { type: "user", user_id: "user-1" },
      is_enabled: false,
      created_at: "2024-08-01T00:00:00Z",
      updated_at: "2024-08-20T00:00:00Z",
    },
    {
      id: "prov-4",
      name: "my-bedrock",
      provider_type: "bedrock",
      base_url: "",
      has_api_key: false,
      config: {
        region: "us-east-1",
        credentials: { type: "static" },
      },
      models: ["anthropic.claude-3-5-sonnet-20241022-v2:0", "amazon.titan-text-express-v1"],
      owner: { type: "user", user_id: "user-1" },
      is_enabled: true,
      created_at: "2024-09-01T00:00:00Z",
      updated_at: "2024-09-01T00:00:00Z",
    },
    {
      id: "prov-5",
      name: "my-vertex",
      provider_type: "vertex",
      base_url: "",
      has_api_key: false,
      config: {
        project: "acme-ml-prod",
        region: "us-central1",
        credentials: { type: "service_account_json" },
      },
      models: ["gemini-2.0-flash", "gemini-1.5-pro"],
      owner: { type: "user", user_id: "user-1" },
      is_enabled: true,
      created_at: "2024-09-15T00:00:00Z",
      updated_at: "2024-09-15T00:00:00Z",
    },
  ],
  pagination: {
    limit: 100,
    has_more: false,
    next_cursor: null,
    prev_cursor: null,
  },
};

const mockBuiltInProviders = {
  data: [
    {
      name: "openrouter",
      provider_type: "open_ai",
      base_url: "https://openrouter.ai/api/v1",
    },
    {
      name: "anthropic-direct",
      provider_type: "anthropic",
      base_url: "https://api.anthropic.com",
    },
    {
      name: "local",
      provider_type: "open_ai",
      base_url: "http://localhost:11434/v1",
    },
  ],
};

const defaultHandlers = [
  http.get("*/api/admin/v1/me/providers", () => {
    return HttpResponse.json(mockProviders);
  }),
  http.get("*/api/admin/v1/me/built-in-providers", () => {
    return HttpResponse.json(mockBuiltInProviders);
  }),
  http.post("*/api/admin/v1/me/providers/test-credentials", async () => {
    await new Promise((resolve) => setTimeout(resolve, 800));
    return HttpResponse.json({
      status: "ok",
      message: "Listed 42 models successfully",
      latency_ms: 234,
    });
  }),
];

const meta: Meta<typeof MyProvidersPage> = {
  title: "Pages/MyProvidersPage",
  component: MyProvidersPage,
  decorators: [
    (Story) => {
      const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
      });
      return (
        <QueryClientProvider client={queryClient}>
          <MemoryRouter>
            <ToastProvider>
              <ConfirmDialogProvider>
                <Story />
              </ConfirmDialogProvider>
            </ToastProvider>
          </MemoryRouter>
        </QueryClientProvider>
      );
    },
  ],
  parameters: {
    layout: "fullscreen",
    msw: {
      handlers: defaultHandlers,
    },
  },
};

export default meta;
type Story = StoryObj<typeof MyProvidersPage>;

export const Default: Story = {};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/me/providers", async () => {
          await new Promise((resolve) => setTimeout(resolve, 999999));
          return HttpResponse.json(mockProviders);
        }),
        http.get("*/api/admin/v1/me/built-in-providers", async () => {
          await new Promise((resolve) => setTimeout(resolve, 999999));
          return HttpResponse.json(mockBuiltInProviders);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/me/providers", () => {
          return HttpResponse.json({
            data: [],
            pagination: { limit: 100, has_more: false, next_cursor: null, prev_cursor: null },
          });
        }),
        http.get("*/api/admin/v1/me/built-in-providers", () => {
          return HttpResponse.json(mockBuiltInProviders);
        }),
      ],
    },
  },
};

export const NoBuiltInProviders: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/me/providers", () => {
          return HttpResponse.json(mockProviders);
        }),
        http.get("*/api/admin/v1/me/built-in-providers", () => {
          return HttpResponse.json({ data: [] });
        }),
      ],
    },
  },
};

export const SingleProvider: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/me/providers", () => {
          return HttpResponse.json({
            data: [mockProviders.data[0]],
            pagination: {
              limit: 100,
              has_more: false,
              next_cursor: null,
              prev_cursor: null,
            },
          });
        }),
        http.get("*/api/admin/v1/me/built-in-providers", () => {
          return HttpResponse.json(mockBuiltInProviders);
        }),
      ],
    },
  },
};

export const ManyModels: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/me/providers", () => {
          return HttpResponse.json({
            data: [
              {
                ...mockProviders.data[0],
                models: [
                  "gpt-4o",
                  "gpt-4o-mini",
                  "gpt-4-turbo",
                  "gpt-3.5-turbo",
                  "o1-preview",
                  "o1-mini",
                  "dall-e-3",
                  "whisper-1",
                ],
              },
            ],
            pagination: {
              limit: 100,
              has_more: false,
              next_cursor: null,
              prev_cursor: null,
            },
          });
        }),
        http.get("*/api/admin/v1/me/built-in-providers", () => {
          return HttpResponse.json(mockBuiltInProviders);
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/api/admin/v1/me/providers", () => {
          return HttpResponse.error();
        }),
        http.get("*/api/admin/v1/me/built-in-providers", () => {
          return HttpResponse.json(mockBuiltInProviders);
        }),
      ],
    },
  },
};
