import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import MyUsagePage from "./MyUsagePage";
import type {
  UsageSummaryResponse,
  DailySpendResponse,
  ModelSpendResponse,
  ProviderSpendResponse,
  PricingSourceSpendResponse,
} from "@/api/generated/types.gen";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: false,
      staleTime: Infinity,
    },
  },
});

const mockSummary: UsageSummaryResponse = {
  total_cost: 47.82,
  total_tokens: 1_250_000,
  input_tokens: 850_000,
  output_tokens: 400_000,
  request_count: 342,
  first_request_at: "2024-05-01T08:15:00Z",
  last_request_at: "2024-06-15T16:42:00Z",
};

const mockByDate: DailySpendResponse[] = Array.from({ length: 30 }, (_, i) => {
  const date = new Date(2024, 4, i + 1);
  const cost = 0.5 + Math.random() * 3;
  const requests = 5 + Math.floor(Math.random() * 20);
  const inputTokens = 10000 + Math.floor(Math.random() * 40000);
  const outputTokens = 5000 + Math.floor(Math.random() * 20000);
  return {
    date: date.toISOString().split("T")[0],
    total_cost: Math.round(cost * 100) / 100,
    total_tokens: inputTokens + outputTokens,
    input_tokens: inputTokens,
    output_tokens: outputTokens,
    request_count: requests,
  };
});

const mockByModel: ModelSpendResponse[] = [
  {
    model: "gpt-4o",
    total_cost: 28.5,
    total_tokens: 750_000,
    input_tokens: 500_000,
    output_tokens: 250_000,
    request_count: 180,
  },
  {
    model: "claude-3.5-sonnet",
    total_cost: 14.2,
    total_tokens: 350_000,
    input_tokens: 240_000,
    output_tokens: 110_000,
    request_count: 95,
  },
  {
    model: "gpt-4o-mini",
    total_cost: 5.12,
    total_tokens: 150_000,
    input_tokens: 110_000,
    output_tokens: 40_000,
    request_count: 67,
  },
];

const mockByProvider: ProviderSpendResponse[] = [
  {
    provider: "openai",
    total_cost: 33.62,
    total_tokens: 900_000,
    input_tokens: 610_000,
    output_tokens: 290_000,
    request_count: 247,
  },
  {
    provider: "anthropic",
    total_cost: 14.2,
    total_tokens: 350_000,
    input_tokens: 240_000,
    output_tokens: 110_000,
    request_count: 95,
  },
];

const mockByPricingSource: PricingSourceSpendResponse[] = [
  {
    pricing_source: "catalog",
    total_cost: 40.5,
    total_tokens: 1_050_000,
    input_tokens: 720_000,
    output_tokens: 330_000,
    request_count: 290,
  },
  {
    pricing_source: "manual",
    total_cost: 7.32,
    total_tokens: 200_000,
    input_tokens: 130_000,
    output_tokens: 70_000,
    request_count: 52,
  },
];

function usageHandlers() {
  return [
    http.get("*/admin/v1/me/usage", () => {
      return HttpResponse.json(mockSummary);
    }),
    http.get("*/admin/v1/me/usage/daily", () => {
      return HttpResponse.json(mockByDate);
    }),
    http.get("*/admin/v1/me/usage/models", () => {
      return HttpResponse.json(mockByModel);
    }),
    http.get("*/admin/v1/me/usage/providers", () => {
      return HttpResponse.json(mockByProvider);
    }),
    http.get("*/admin/v1/me/usage/daily-models", () => {
      return HttpResponse.json([]);
    }),
    http.get("*/admin/v1/me/usage/daily-providers", () => {
      return HttpResponse.json([]);
    }),
    http.get("*/admin/v1/me/usage/pricing-sources", () => {
      return HttpResponse.json(mockByPricingSource);
    }),
    http.get("*/admin/v1/me/usage/daily-pricing-sources", () => {
      return HttpResponse.json([]);
    }),
  ];
}

const meta: Meta<typeof MyUsagePage> = {
  title: "Pages/MyUsagePage",
  component: MyUsagePage,
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
        <MemoryRouter initialEntries={["/usage"]}>
          <Routes>
            <Route path="/usage" element={<Story />} />
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
      handlers: usageHandlers(),
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/me/usage", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockSummary);
        }),
        http.get("*/admin/v1/me/usage/daily", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockByDate);
        }),
        http.get("*/admin/v1/me/usage/models", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockByModel);
        }),
        http.get("*/admin/v1/me/usage/providers", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockByProvider);
        }),
        http.get("*/admin/v1/me/usage/pricing-sources", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockByPricingSource);
        }),
        http.get("*/admin/v1/me/usage/daily-models", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json([]);
        }),
        http.get("*/admin/v1/me/usage/daily-providers", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json([]);
        }),
        http.get("*/admin/v1/me/usage/daily-pricing-sources", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json([]);
        }),
      ],
    },
  },
};

export const Empty: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/me/usage", () => {
          return HttpResponse.json({
            total_cost: 0,
            total_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
            request_count: 0,
            first_request_at: null,
            last_request_at: null,
          });
        }),
        http.get("*/admin/v1/me/usage/daily", () => {
          return HttpResponse.json([]);
        }),
        http.get("*/admin/v1/me/usage/models", () => {
          return HttpResponse.json([]);
        }),
        http.get("*/admin/v1/me/usage/providers", () => {
          return HttpResponse.json([]);
        }),
        http.get("*/admin/v1/me/usage/daily-models", () => {
          return HttpResponse.json([]);
        }),
        http.get("*/admin/v1/me/usage/daily-providers", () => {
          return HttpResponse.json([]);
        }),
        http.get("*/admin/v1/me/usage/pricing-sources", () => {
          return HttpResponse.json([]);
        }),
        http.get("*/admin/v1/me/usage/daily-pricing-sources", () => {
          return HttpResponse.json([]);
        }),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/me/usage", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/me/usage/daily", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/me/usage/models", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/me/usage/providers", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/me/usage/daily-models", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/me/usage/daily-providers", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/me/usage/pricing-sources", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/me/usage/daily-pricing-sources", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Database connection failed" } },
            { status: 500 }
          );
        }),
      ],
    },
  },
};
