import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import ProviderHealthPage from "./ProviderHealthPage";
import type {
  ProviderHealthResponse,
  CircuitBreakersResponse,
  ProviderHealthState,
  CircuitBreakerStatus,
  ProviderStatsResponse,
  ProviderStats,
  ProviderStatsHistorical,
  TimeBucketStats,
} from "@/api/generated/types.gen";
import { ToastProvider } from "@/components/Toast/Toast";

// Create a new QueryClient for each story to prevent cache sharing
const createQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        staleTime: Infinity,
      },
    },
  });

const mockHealthyProviders: ProviderHealthState[] = [
  {
    provider: "openai",
    status: "healthy",
    latency_ms: 120,
    last_check: "2024-06-15T10:30:00Z",
    consecutive_failures: 0,
    consecutive_successes: 15,
  },
  {
    provider: "anthropic",
    status: "healthy",
    latency_ms: 95,
    last_check: "2024-06-15T10:30:00Z",
    consecutive_failures: 0,
    consecutive_successes: 42,
  },
  {
    provider: "azure-openai",
    status: "healthy",
    latency_ms: 180,
    last_check: "2024-06-15T10:30:00Z",
    consecutive_failures: 0,
    consecutive_successes: 8,
  },
];

const mockMixedProviders: ProviderHealthState[] = [
  {
    provider: "openai",
    status: "healthy",
    latency_ms: 120,
    last_check: "2024-06-15T10:30:00Z",
    consecutive_failures: 0,
    consecutive_successes: 15,
  },
  {
    provider: "anthropic",
    status: "unhealthy",
    latency_ms: 5000,
    error: "Connection timeout after 5000ms",
    last_check: "2024-06-15T10:30:00Z",
    consecutive_failures: 3,
    consecutive_successes: 0,
  },
  {
    provider: "azure-openai",
    status: "unknown",
    latency_ms: 0,
    last_check: "2024-06-15T10:25:00Z",
    consecutive_failures: 0,
    consecutive_successes: 0,
  },
  {
    provider: "bedrock",
    status: "unhealthy",
    latency_ms: 2500,
    error: "503 Service Unavailable",
    status_code: 503,
    last_check: "2024-06-15T10:30:00Z",
    consecutive_failures: 5,
    consecutive_successes: 0,
  },
];

const mockCircuitBreakers: CircuitBreakerStatus[] = [
  {
    provider: "openai",
    state: "closed",
    failure_count: 0,
  },
  {
    provider: "anthropic",
    state: "open",
    failure_count: 5,
  },
  {
    provider: "azure-openai",
    state: "half_open",
    failure_count: 2,
  },
  {
    provider: "bedrock",
    state: "closed",
    failure_count: 1,
  },
];

const mockStats: ProviderStats[] = [
  {
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
  },
  {
    provider: "anthropic",
    request_count: 8320,
    error_count: 156,
    p50_latency_ms: 210,
    p95_latency_ms: 580,
    p99_latency_ms: 1250,
    input_tokens: 1800000,
    output_tokens: 950000,
    total_cost_microcents: 3200000000, // $32.00
    last_updated: "2024-06-15T10:30:00Z",
  },
  {
    provider: "azure-openai",
    request_count: 4210,
    error_count: 12,
    p50_latency_ms: 180,
    p95_latency_ms: 420,
    p99_latency_ms: 980,
    input_tokens: 890000,
    output_tokens: 450000,
    total_cost_microcents: 1500000000, // $15.00
    last_updated: "2024-06-15T10:30:00Z",
  },
];

const mockMixedStats: ProviderStats[] = [
  {
    provider: "openai",
    request_count: 15420,
    error_count: 23,
    p50_latency_ms: 145,
    p95_latency_ms: 320,
    p99_latency_ms: 890,
    input_tokens: 2500000,
    output_tokens: 1200000,
    total_cost_microcents: 4500000000,
    last_updated: "2024-06-15T10:30:00Z",
  },
  {
    provider: "anthropic",
    request_count: 8320,
    error_count: 850, // High error count for unhealthy provider
    p50_latency_ms: 2100,
    p95_latency_ms: 4200,
    p99_latency_ms: 5000,
    input_tokens: 1800000,
    output_tokens: 950000,
    total_cost_microcents: 3200000000,
    last_updated: "2024-06-15T10:30:00Z",
  },
  {
    provider: "bedrock",
    request_count: 3200,
    error_count: 320,
    p50_latency_ms: 1800,
    p95_latency_ms: 2800,
    p99_latency_ms: 3500,
    input_tokens: 650000,
    output_tokens: 320000,
    total_cost_microcents: 980000000,
    last_updated: "2024-06-15T10:30:00Z",
  },
];

const mockHealthResponse: ProviderHealthResponse = {
  providers: mockHealthyProviders,
};

const mockMixedHealthResponse: ProviderHealthResponse = {
  providers: mockMixedProviders,
};

const mockCircuitResponse: CircuitBreakersResponse = {
  circuit_breakers: mockCircuitBreakers,
};

const emptyHealthResponse: ProviderHealthResponse = {
  providers: [],
};

const emptyCircuitResponse: CircuitBreakersResponse = {
  circuit_breakers: [],
};

const mockStatsResponse: ProviderStatsResponse = {
  stats: mockStats,
};

const mockMixedStatsResponse: ProviderStatsResponse = {
  stats: mockMixedStats,
};

const emptyStatsResponse: ProviderStatsResponse = {
  stats: [],
};

// Helper function to generate mock history data
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

// Handler for history endpoint
const createHistoryHandler = () =>
  http.get("*/admin/v1/providers/:provider/stats/history", ({ request, params }) => {
    const provider = params.provider as string;
    const url = new URL(request.url);
    const granularity = (url.searchParams.get("granularity") || "hour") as "hour" | "day";
    const hours = granularity === "day" ? 720 : 24;

    return HttpResponse.json(generateMockHistoryData(provider, hours, granularity));
  });

const meta: Meta<typeof ProviderHealthPage> = {
  title: "Admin/ProviderHealthPage",
  component: ProviderHealthPage,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [{ id: "heading-order", enabled: false }],
      },
    },
  },
  decorators: [
    (Story) => {
      // Create a fresh QueryClient for each story to prevent cache sharing
      const queryClient = createQueryClient();
      return (
        <QueryClientProvider client={queryClient}>
          <ToastProvider>
            <MemoryRouter initialEntries={["/admin/provider-health"]}>
              <Routes>
                <Route path="/admin/provider-health" element={<Story />} />
              </Routes>
            </MemoryRouter>
          </ToastProvider>
        </QueryClientProvider>
      );
    },
  ],
};

export default meta;
type Story = StoryObj<typeof meta>;

export const AllHealthy: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", () => {
          return HttpResponse.json(mockHealthResponse);
        }),
        http.get("*/admin/v1/providers/circuit-breakers", () => {
          return HttpResponse.json({
            circuit_breakers: mockHealthyProviders.map((p) => ({
              provider: p.provider,
              state: "closed",
              failure_count: 0,
            })),
          });
        }),
        http.get("*/admin/v1/providers/stats", () => {
          return HttpResponse.json(mockStatsResponse);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const MixedStatus: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", () => {
          return HttpResponse.json(mockMixedHealthResponse);
        }),
        http.get("*/admin/v1/providers/circuit-breakers", () => {
          return HttpResponse.json(mockCircuitResponse);
        }),
        http.get("*/admin/v1/providers/stats", () => {
          return HttpResponse.json(mockMixedStatsResponse);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const NoProviders: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", () => {
          return HttpResponse.json(emptyHealthResponse);
        }),
        http.get("*/admin/v1/providers/circuit-breakers", () => {
          return HttpResponse.json(emptyCircuitResponse);
        }),
        http.get("*/admin/v1/providers/stats", () => {
          return HttpResponse.json(emptyStatsResponse);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const Loading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockHealthResponse);
        }),
        http.get("*/admin/v1/providers/circuit-breakers", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockCircuitResponse);
        }),
        http.get("*/admin/v1/providers/stats", async () => {
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockStatsResponse);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const Error: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Failed to fetch provider health" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/providers/circuit-breakers", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Failed to fetch circuit breakers" } },
            { status: 500 }
          );
        }),
        http.get("*/admin/v1/providers/stats", () => {
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Failed to fetch provider stats" } },
            { status: 500 }
          );
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const ManyProviders: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", () => {
          const manyProviders: ProviderHealthState[] = Array.from({ length: 15 }, (_, i) => ({
            provider: `provider-${i + 1}`,
            status: i % 5 === 0 ? "unhealthy" : i % 7 === 0 ? "unknown" : "healthy",
            latency_ms: 50 + Math.floor(Math.random() * 200),
            error: i % 5 === 0 ? `Error for provider ${i + 1}` : undefined,
            last_check: "2024-06-15T10:30:00Z",
            consecutive_failures: i % 5 === 0 ? (i % 3) + 1 : 0,
            consecutive_successes: i % 5 === 0 ? 0 : Math.floor(Math.random() * 50),
          }));
          return HttpResponse.json({ providers: manyProviders });
        }),
        http.get("*/admin/v1/providers/circuit-breakers", () => {
          const manyCircuits: CircuitBreakerStatus[] = Array.from({ length: 15 }, (_, i) => ({
            provider: `provider-${i + 1}`,
            state: i % 5 === 0 ? "open" : i % 7 === 0 ? "half_open" : "closed",
            failure_count: i % 5 === 0 ? (i % 3) + 1 : 0,
          }));
          return HttpResponse.json({ circuit_breakers: manyCircuits });
        }),
        http.get("*/admin/v1/providers/stats", () => {
          const manyStats: ProviderStats[] = Array.from({ length: 15 }, (_, i) => ({
            provider: `provider-${i + 1}`,
            request_count: Math.floor(Math.random() * 20000) + 500,
            error_count:
              i % 5 === 0 ? Math.floor(Math.random() * 500) + 50 : Math.floor(Math.random() * 30),
            p50_latency_ms: 100 + Math.floor(Math.random() * 200),
            p95_latency_ms: 300 + Math.floor(Math.random() * 400),
            p99_latency_ms: 700 + Math.floor(Math.random() * 800),
            input_tokens: Math.floor(Math.random() * 3000000) + 100000,
            output_tokens: Math.floor(Math.random() * 1500000) + 50000,
            total_cost_microcents: Math.floor(Math.random() * 5000000000) + 100000000,
            last_updated: "2024-06-15T10:30:00Z",
          }));
          return HttpResponse.json({ stats: manyStats });
        }),
        createHistoryHandler(),
      ],
    },
  },
};

// Additional stories for stats-specific scenarios
export const StatsLoading: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", () => {
          return HttpResponse.json(mockHealthResponse);
        }),
        http.get("*/admin/v1/providers/circuit-breakers", () => {
          return HttpResponse.json({
            circuit_breakers: mockHealthyProviders.map((p) => ({
              provider: p.provider,
              state: "closed",
              failure_count: 0,
            })),
          });
        }),
        http.get("*/admin/v1/providers/stats", async () => {
          // Stats endpoint delays while health loads immediately
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json(mockStatsResponse);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const StatsError: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", () => {
          return HttpResponse.json(mockHealthResponse);
        }),
        http.get("*/admin/v1/providers/circuit-breakers", () => {
          return HttpResponse.json({
            circuit_breakers: mockHealthyProviders.map((p) => ({
              provider: p.provider,
              state: "closed",
              failure_count: 0,
            })),
          });
        }),
        http.get("*/admin/v1/providers/stats", () => {
          // Stats endpoint fails but health shows
          return HttpResponse.json(
            { error: { code: "internal_error", message: "Failed to fetch stats" } },
            { status: 500 }
          );
        }),
        createHistoryHandler(),
      ],
    },
  },
};

export const NoStats: Story = {
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", () => {
          return HttpResponse.json(mockHealthResponse);
        }),
        http.get("*/admin/v1/providers/circuit-breakers", () => {
          return HttpResponse.json({
            circuit_breakers: mockHealthyProviders.map((p) => ({
              provider: p.provider,
              state: "closed",
              failure_count: 0,
            })),
          });
        }),
        http.get("*/admin/v1/providers/stats", () => {
          // Empty stats array
          return HttpResponse.json(emptyStatsResponse);
        }),
        createHistoryHandler(),
      ],
    },
  },
};

/**
 * Demonstrates the historical charts feature.
 * Click on a provider name in the table to view its historical trends.
 * The charts show latency percentiles, error rate, and request volume over time.
 */
export const WithChartsInteraction: Story = {
  parameters: {
    docs: {
      description: {
        story:
          "Click on a provider name (e.g., 'openai') in the health table to view historical charts. The charts display latency percentiles (P50/P95/P99), error rate, and request volume. Use the time range buttons (1h, 6h, 24h, 7d, 30d) to adjust the view.",
      },
    },
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/health", () => {
          return HttpResponse.json(mockHealthResponse);
        }),
        http.get("*/admin/v1/providers/circuit-breakers", () => {
          return HttpResponse.json({
            circuit_breakers: mockHealthyProviders.map((p) => ({
              provider: p.provider,
              state: "closed",
              failure_count: 0,
            })),
          });
        }),
        http.get("*/admin/v1/providers/stats", () => {
          return HttpResponse.json(mockStatsResponse);
        }),
        createHistoryHandler(),
      ],
    },
  },
};
