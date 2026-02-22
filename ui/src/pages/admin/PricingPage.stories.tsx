import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import PricingPage from "./PricingPage";
import type { DbModelPricing, ModelPricingListResponse } from "@/api/generated/types.gen";
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

const mockPricing: DbModelPricing[] = [
  {
    id: "price-001",
    provider: "openai",
    model: "gpt-4o",
    input_per_1m_tokens: 250000,
    output_per_1m_tokens: 1000000,
    cached_input_per_1m_tokens: 125000,
    cache_write_per_1m_tokens: null,
    reasoning_per_1m_tokens: null,
    per_request: null,
    per_image: null,
    per_1m_characters: null,
    per_second: null,
    owner: { type: "global" },
    source: "manual",
    created_at: "2024-01-15T09:00:00Z",
    updated_at: "2024-06-10T14:30:00Z",
  },
  {
    id: "price-002",
    provider: "openai",
    model: "gpt-4o-mini",
    input_per_1m_tokens: 15000,
    output_per_1m_tokens: 60000,
    cached_input_per_1m_tokens: 7500,
    cache_write_per_1m_tokens: null,
    reasoning_per_1m_tokens: null,
    per_request: null,
    per_image: null,
    per_1m_characters: null,
    per_second: null,
    owner: { type: "global" },
    source: "provider_api",
    created_at: "2024-02-01T11:00:00Z",
    updated_at: "2024-06-01T08:00:00Z",
  },
  {
    id: "price-003",
    provider: "anthropic",
    model: "claude-3.5-sonnet",
    input_per_1m_tokens: 300000,
    output_per_1m_tokens: 1500000,
    cached_input_per_1m_tokens: 150000,
    cache_write_per_1m_tokens: 375000,
    reasoning_per_1m_tokens: null,
    per_request: null,
    per_image: null,
    per_1m_characters: null,
    per_second: null,
    owner: { type: "organization", org_id: "org-123" },
    source: "manual",
    created_at: "2024-03-10T16:00:00Z",
    updated_at: "2024-05-20T09:15:00Z",
  },
  {
    id: "price-004",
    provider: "anthropic",
    model: "claude-3-haiku",
    input_per_1m_tokens: 25000,
    output_per_1m_tokens: 125000,
    cached_input_per_1m_tokens: null,
    cache_write_per_1m_tokens: null,
    reasoning_per_1m_tokens: null,
    per_request: null,
    per_image: null,
    per_1m_characters: null,
    per_second: null,
    owner: { type: "team", team_id: "team-001" },
    source: "default",
    created_at: "2024-04-05T13:30:00Z",
    updated_at: "2024-04-05T13:30:00Z",
  },
  {
    id: "price-005",
    provider: "openai",
    model: "o3-mini",
    input_per_1m_tokens: 110000,
    output_per_1m_tokens: 440000,
    cached_input_per_1m_tokens: 55000,
    cache_write_per_1m_tokens: null,
    reasoning_per_1m_tokens: 440000,
    per_request: null,
    per_image: null,
    per_1m_characters: null,
    per_second: null,
    owner: { type: "global" },
    source: "provider_api",
    created_at: "2024-05-01T07:00:00Z",
    updated_at: "2024-06-12T10:45:00Z",
  },
];

const mockPricingResponse: ModelPricingListResponse = {
  data: mockPricing,
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const emptyPricingResponse: ModelPricingListResponse = {
  data: [],
  pagination: {
    limit: 25,
    has_more: false,
  },
};

const meta: Meta<typeof PricingPage> = {
  title: "Admin/PricingPage",
  component: PricingPage,
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
            <MemoryRouter initialEntries={["/admin/pricing"]}>
              <Routes>
                <Route path="/admin/pricing" element={<Story />} />
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
        http.get("*/admin/v1/model-pricing", () => {
          return HttpResponse.json(mockPricingResponse);
        }),
        http.post("*/admin/v1/model-pricing", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newPricing: DbModelPricing = {
            id: `price-${Date.now()}`,
            provider: body.provider as string,
            model: body.model as string,
            input_per_1m_tokens: body.input_per_1m_tokens as number,
            output_per_1m_tokens: body.output_per_1m_tokens as number,
            cached_input_per_1m_tokens: (body.cached_input_per_1m_tokens as number) || null,
            cache_write_per_1m_tokens: null,
            reasoning_per_1m_tokens: null,
            per_request: null,
            per_image: null,
            per_1m_characters: null,
            per_second: null,
            owner: { type: "global" },
            source: "manual",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newPricing, { status: 201 });
        }),
        http.patch("*/admin/v1/model-pricing/:id", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const updated: DbModelPricing = {
            ...mockPricing[0],
            ...(body as Partial<DbModelPricing>),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(updated);
        }),
        http.delete("*/admin/v1/model-pricing/:id", () => {
          return new HttpResponse(null, { status: 204 });
        }),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/model-pricing", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockPricingResponse);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/model-pricing", () => {
          return HttpResponse.json(emptyPricingResponse);
        }),
        http.post("*/admin/v1/model-pricing", async ({ request }) => {
          const body = (await request.json()) as Record<string, unknown>;
          const newPricing: DbModelPricing = {
            id: `price-${Date.now()}`,
            provider: body.provider as string,
            model: body.model as string,
            input_per_1m_tokens: body.input_per_1m_tokens as number,
            output_per_1m_tokens: body.output_per_1m_tokens as number,
            cached_input_per_1m_tokens: (body.cached_input_per_1m_tokens as number) || null,
            cache_write_per_1m_tokens: null,
            reasoning_per_1m_tokens: null,
            per_request: null,
            per_image: null,
            per_1m_characters: null,
            per_second: null,
            owner: { type: "global" },
            source: "manual",
            created_at: new Date().toISOString(),
            updated_at: new Date().toISOString(),
          };
          return HttpResponse.json(newPricing, { status: 201 });
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/model-pricing", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
      ],
    },
  },
};

export const ManyEntries: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/model-pricing", () => {
          const providers = ["openai", "anthropic", "bedrock", "vertex"];
          const models = [
            "gpt-4o",
            "gpt-4o-mini",
            "claude-3.5-sonnet",
            "claude-3-haiku",
            "llama-3.1-70b",
          ];
          const sources: Array<"manual" | "provider_api" | "default"> = [
            "manual",
            "provider_api",
            "default",
          ];
          const manyPricing: DbModelPricing[] = Array.from({ length: 25 }, (_, i) => ({
            id: `price-${i + 1}`,
            provider: providers[i % providers.length],
            model: models[i % models.length],
            input_per_1m_tokens: (i + 1) * 10000,
            output_per_1m_tokens: (i + 1) * 40000,
            cached_input_per_1m_tokens: i % 3 === 0 ? (i + 1) * 5000 : null,
            cache_write_per_1m_tokens: null,
            reasoning_per_1m_tokens: null,
            per_request: null,
            per_image: null,
            per_1m_characters: null,
            per_second: null,
            owner: i % 4 === 0 ? { type: "global" } : { type: "organization", org_id: "org-123" },
            source: sources[i % sources.length],
            created_at: new Date(2024, 0, i + 1).toISOString(),
            updated_at: new Date(2024, 5, i + 1).toISOString(),
          }));
          return HttpResponse.json({
            data: manyPricing,
            pagination: {
              limit: 25,
              has_more: true,
              next_cursor: "bW9ja19jdXJzb3I=",
            },
          });
        }),
      ],
    },
  },
};
