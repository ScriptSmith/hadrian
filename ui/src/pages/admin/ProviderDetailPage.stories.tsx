import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import ProviderDetailPage from "./ProviderDetailPage";
import type {
  ProviderHealthState,
  CircuitBreakerStatus,
  ProviderStats,
  ProviderStatsHistorical,
  TimeBucketStats,
} from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";

const createQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        staleTime: Infinity,
      },
    },
  });

const mockHealthyProvider: ProviderHealthState = {
  provider: "openai",
  status: "healthy",
  latency_ms: 125,
  last_check: "2024-06-15T10:30:00Z",
  consecutive_failures: 0,
  consecutive_successes: 42,
};

const mockUnhealthyProvider: ProviderHealthState = {
  provider: "anthropic",
  status: "unhealthy",
  latency_ms: 5000,
  error: "Connection timeout after 5000ms",
  last_check: "2024-06-15T10:30:00Z",
  consecutive_failures: 5,
  consecutive_successes: 0,
};

const mockClosedCircuit: CircuitBreakerStatus = {
  provider: "openai",
  state: "closed",
  failure_count: 0,
};

const mockOpenCircuit: CircuitBreakerStatus = {
  provider: "anthropic",
  state: "open",
  failure_count: 5,
};

const mockHalfOpenCircuit: CircuitBreakerStatus = {
  provider: "azure-openai",
  state: "half_open",
  failure_count: 2,
};

const mockStats: ProviderStats = {
  provider: "openai",
  request_count: 15420,
  error_count: 23,
  p50_latency_ms: 145,
  p95_latency_ms: 320,
  p99_latency_ms: 890,
  input_tokens: 2500000,
  output_tokens: 1200000,
  total_cost_microcents: 4500000000, // $45.00
  last_updated: "2024-06-15T10:30:00Z",
};

const mockHighErrorStats: ProviderStats = {
  provider: "anthropic",
  request_count: 8320,
  error_count: 850,
  p50_latency_ms: 2100,
  p95_latency_ms: 4200,
  p99_latency_ms: 5000,
  input_tokens: 1800000,
  output_tokens: 950000,
  total_cost_microcents: 3200000000,
  last_updated: "2024-06-15T10:30:00Z",
};

function generateMockHistoryData(
  provider: string,
  hours: number,
  granularity: "hour" | "day"
): ProviderStatsHistorical {
  const bucketDuration = granularity === "hour" ? 3600 : 86400;
  const numBuckets = granularity === "hour" ? hours : Math.ceil(hours / 24);

  const data: TimeBucketStats[] = [];
  for (let i = numBuckets - 1; i >= 0; i--) {
    const bucketStart = new Date(Date.now() - i * bucketDuration * 1000);
    const baseLatency = 100 + Math.random() * 80;
    const requestCount = Math.floor(400 + Math.random() * 800);
    const errorCount = Math.floor(requestCount * (Math.random() * 0.04));

    data.push({
      bucket_start: bucketStart.toISOString(),
      bucket_duration_secs: bucketDuration,
      request_count: requestCount,
      error_count: errorCount,
      p50_latency_ms: baseLatency,
      p95_latency_ms: baseLatency * 1.6,
      p99_latency_ms: baseLatency * 2.4,
      avg_latency_ms: baseLatency * 1.15,
      total_tokens: requestCount * 180,
      total_cost_microcents: requestCount * 65,
    });
  }

  return {
    provider,
    granularity,
    data,
  };
}

const createHistoryHandler = () =>
  http.get("*/admin/v1/providers/:provider/stats/history", ({ request, params }) => {
    const provider = params.provider as string;
    const url = new URL(request.url);
    const granularity = (url.searchParams.get("granularity") || "hour") as "hour" | "day";
    const hours = granularity === "day" ? 720 : 24;

    return HttpResponse.json(generateMockHistoryData(provider, hours, granularity));
  });

// Create a decorator factory for different provider routes
const createDecorator = (providerName: string) => (Story: React.ComponentType) => {
  const queryClient = createQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <MemoryRouter initialEntries={[`/admin/provider-health/${providerName}`]}>
          <Routes>
            <Route path="/admin/provider-health/:providerName" element={<Story />} />
            <Route path="/admin/provider-health" element={<div>Provider Health List Page</div>} />
          </Routes>
        </MemoryRouter>
      </ToastProvider>
    </QueryClientProvider>
  );
};

const meta: Meta<typeof ProviderDetailPage> = {
  title: "Admin/ProviderDetailPage",
  component: ProviderDetailPage,
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

export const Healthy: Story = {
  decorators: [createDecorator("openai")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/health", () => {
          return HttpResponse.json(mockHealthyProvider);
        }),
        http.get("*/admin/v1/providers/:provider/circuit-breaker", () => {
          return HttpResponse.json(mockClosedCircuit);
        }),
        http.get("*/admin/v1/providers/:provider/stats", () => {
          return HttpResponse.json(mockStats);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const Unhealthy: Story = {
  decorators: [createDecorator("anthropic")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/health", () => {
          return HttpResponse.json(mockUnhealthyProvider);
        }),
        http.get("*/admin/v1/providers/:provider/circuit-breaker", () => {
          return HttpResponse.json(mockOpenCircuit);
        }),
        http.get("*/admin/v1/providers/:provider/stats", () => {
          return HttpResponse.json(mockHighErrorStats);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const HalfOpenCircuit: Story = {
  decorators: [createDecorator("azure-openai")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/health", () => {
          return HttpResponse.json({
            provider: "azure-openai",
            status: "healthy",
            latency_ms: 180,
            last_check: "2024-06-15T10:30:00Z",
            consecutive_failures: 0,
            consecutive_successes: 3,
          });
        }),
        http.get("*/admin/v1/providers/:provider/circuit-breaker", () => {
          return HttpResponse.json(mockHalfOpenCircuit);
        }),
        http.get("*/admin/v1/providers/:provider/stats", () => {
          return HttpResponse.json({
            ...mockStats,
            provider: "azure-openai",
            request_count: 4210,
            error_count: 42,
          });
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const Loading: Story = {
  decorators: [createDecorator("openai")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/health", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockHealthyProvider);
        }),
        http.get("*/admin/v1/providers/:provider/circuit-breaker", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockClosedCircuit);
        }),
        http.get("*/admin/v1/providers/:provider/stats", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockStats);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const StatsLoading: Story = {
  decorators: [createDecorator("openai")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/health", () => {
          return HttpResponse.json(mockHealthyProvider);
        }),
        http.get("*/admin/v1/providers/:provider/circuit-breaker", () => {
          return HttpResponse.json(mockClosedCircuit);
        }),
        http.get("*/admin/v1/providers/:provider/stats", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockStats);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const Error: Story = {
  decorators: [createDecorator("openai")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/health", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "Provider not found" } },
            { status: 404 }
          );
        }),
        http.get("*/admin/v1/providers/:provider/circuit-breaker", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "Provider not found" } },
            { status: 404 }
          );
        }),
        http.get("*/admin/v1/providers/:provider/stats", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "Provider not found" } },
            { status: 404 }
          );
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const NoStats: Story = {
  decorators: [createDecorator("openai")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/health", () => {
          return HttpResponse.json(mockHealthyProvider);
        }),
        http.get("*/admin/v1/providers/:provider/circuit-breaker", () => {
          return HttpResponse.json(mockClosedCircuit);
        }),
        http.get("*/admin/v1/providers/:provider/stats", () => {
          return HttpResponse.json(
            { error: { code: "not_found", message: "No stats available" } },
            { status: 404 }
          );
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const LargeNumbers: Story = {
  decorators: [createDecorator("openai")],
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/health", () => {
          return HttpResponse.json(mockHealthyProvider);
        }),
        http.get("*/admin/v1/providers/:provider/circuit-breaker", () => {
          return HttpResponse.json(mockClosedCircuit);
        }),
        http.get("*/admin/v1/providers/:provider/stats", () => {
          return HttpResponse.json({
            provider: "openai",
            request_count: 15420000,
            error_count: 2300,
            p50_latency_ms: 145,
            p95_latency_ms: 320,
            p99_latency_ms: 890,
            input_tokens: 2500000000,
            output_tokens: 1200000000,
            total_cost_microcents: 450000000000, // $4,500.00
            last_updated: "2024-06-15T10:30:00Z",
          });
        }),
        createHistoryHandler(),
      ],
    },
  },
};
