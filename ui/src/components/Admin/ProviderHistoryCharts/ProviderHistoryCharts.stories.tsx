import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { http, HttpResponse } from "msw";
import { ProviderHistoryCharts } from "./ProviderHistoryCharts";
import type { ProviderStatsHistorical, TimeBucketStats } from "@/api/generated/types.gen";

// Create a fresh QueryClient for each story
const createQueryClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        refetchOnWindowFocus: false,
      },
    },
  });

// Generate mock data for a time range
function generateMockData(hours: number, granularity: "hour" | "day"): TimeBucketStats[] {
  const data: TimeBucketStats[] = [];
  const bucketDuration = granularity === "hour" ? 3600 : 86400;
  const numBuckets = granularity === "hour" ? hours : Math.ceil(hours / 24);

  for (let i = numBuckets - 1; i >= 0; i--) {
    const bucketStart = new Date(Date.now() - i * bucketDuration * 1000);
    const baseLatency = 100 + Math.random() * 50;
    const requestCount = Math.floor(500 + Math.random() * 1000);
    const errorCount = Math.floor(requestCount * (Math.random() * 0.03));

    data.push({
      bucket_start: bucketStart.toISOString(),
      bucket_duration_secs: bucketDuration,
      request_count: requestCount,
      error_count: errorCount,
      p50_latency_ms: baseLatency,
      p95_latency_ms: baseLatency * 1.5,
      p99_latency_ms: baseLatency * 2.2,
      avg_latency_ms: baseLatency * 1.1,
      total_tokens: requestCount * 150,
      total_cost_microcents: requestCount * 50,
    });
  }

  return data;
}

// Generate mock data with some missing latency values
function generateSparseData(hours: number): TimeBucketStats[] {
  const data = generateMockData(hours, "hour");
  // Randomly remove some latency data points
  return data.map((bucket, i) => ({
    ...bucket,
    p50_latency_ms: i % 3 === 0 ? null : bucket.p50_latency_ms,
    p95_latency_ms: i % 4 === 0 ? null : bucket.p95_latency_ms,
    p99_latency_ms: i % 5 === 0 ? null : bucket.p99_latency_ms,
  }));
}

// Generate high error rate data
function generateHighErrorData(hours: number): TimeBucketStats[] {
  const data = generateMockData(hours, "hour");
  return data.map((bucket) => ({
    ...bucket,
    error_count: Math.floor(bucket.request_count * (0.05 + Math.random() * 0.15)),
  }));
}

const meta: Meta<typeof ProviderHistoryCharts> = {
  title: "Components/Admin/ProviderHistoryCharts",
  component: ProviderHistoryCharts,
  parameters: {
    layout: "padded",
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={createQueryClient()}>
        <div className="max-w-5xl">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
};

export default meta;

type Story = StoryObj<typeof ProviderHistoryCharts>;

export const Default: Story = {
  args: {
    provider: "openai",
    onClose: () => console.log("Close clicked"),
  },
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/stats/history", ({ request }) => {
          const url = new URL(request.url);
          const granularity = (url.searchParams.get("granularity") || "hour") as "hour" | "day";
          const hours = granularity === "day" ? 720 : 24;

          const response: ProviderStatsHistorical = {
            provider: "openai",
            granularity,
            data: generateMockData(hours, granularity),
          };
          return HttpResponse.json(response);
        }),
      ],
    },
  },
};

export const WithSparseData: Story = {
  args: {
    provider: "anthropic",
    onClose: () => console.log("Close clicked"),
  },
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/stats/history", () => {
          const response: ProviderStatsHistorical = {
            provider: "anthropic",
            granularity: "hour",
            data: generateSparseData(24),
          };
          return HttpResponse.json(response);
        }),
      ],
    },
  },
};

export const HighErrorRate: Story = {
  args: {
    provider: "bedrock",
    onClose: () => console.log("Close clicked"),
  },
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/stats/history", () => {
          const response: ProviderStatsHistorical = {
            provider: "bedrock",
            granularity: "hour",
            data: generateHighErrorData(24),
          };
          return HttpResponse.json(response);
        }),
      ],
    },
  },
};

export const NoData: Story = {
  args: {
    provider: "vertex",
    onClose: () => console.log("Close clicked"),
  },
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/stats/history", () => {
          const response: ProviderStatsHistorical = {
            provider: "vertex",
            granularity: "hour",
            data: [],
          };
          return HttpResponse.json(response);
        }),
      ],
    },
  },
};

export const NoLatencyData: Story = {
  args: {
    provider: "azure",
    onClose: () => console.log("Close clicked"),
  },
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/stats/history", () => {
          const response: ProviderStatsHistorical = {
            provider: "azure",
            granularity: "hour",
            data: generateMockData(24, "hour").map((bucket) => ({
              ...bucket,
              p50_latency_ms: null,
              p95_latency_ms: null,
              p99_latency_ms: null,
              avg_latency_ms: null,
            })),
          };
          return HttpResponse.json(response);
        }),
      ],
    },
  },
};

export const Loading: Story = {
  args: {
    provider: "openai",
  },
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/stats/history", async () => {
          // Simulate a very slow response
          await new Promise((resolve) => setTimeout(resolve, 100000));
          return HttpResponse.json({});
        }),
      ],
    },
  },
};

export const WithoutCloseButton: Story = {
  args: {
    provider: "openai",
    // onClose is not provided
  },
  parameters: {
    msw: {
      handlers: [
        http.get("*/admin/v1/providers/:provider/stats/history", () => {
          const response: ProviderStatsHistorical = {
            provider: "openai",
            granularity: "hour",
            data: generateMockData(24, "hour"),
          };
          return HttpResponse.json(response);
        }),
      ],
    },
  },
};
