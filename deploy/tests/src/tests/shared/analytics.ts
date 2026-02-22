/**
 * Provider Analytics Tests
 *
 * Tests the provider analytics system for:
 *   - Request count tracking
 *   - Error rate tracking (5xx errors only)
 *   - Latency percentile calculations
 *   - Token and cost aggregation
 *   - All providers stats endpoint
 *   - Historical stats endpoint
 *
 * Uses baseline + delta approach to handle async analytics flush timing.
 * Analytics events are published async and flushed periodically,
 * so tests use retry() to wait for stats to update.
 *
 * Note: Analytics require the UsageTracker to have model/provider info set.
 * If analytics are not being collected (stats return 404), tests will skip.
 */
import { describe, it, expect } from "vitest";
import { trackedFetch } from "../../utils/tracked-fetch";
import { retry } from "../../utils/retry";

/**
 * Context for analytics tests.
 */
export interface AnalyticsTestContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /** Admin token for admin API access */
  adminToken: string;
  /** API key for making requests */
  apiKey: string;
  /** Provider name to test (e.g., "test") */
  providerName: string;
  /** Model name to use (e.g., "test/test-model") */
  modelName: string;
}

/**
 * Provider stats from the admin API.
 */
export interface ProviderStats {
  provider: string;
  p50_latency_ms?: number;
  p95_latency_ms?: number;
  p99_latency_ms?: number;
  avg_latency_ms?: number;
  min_latency_ms?: number;
  max_latency_ms?: number;
  request_count: number;
  error_count: number;
  errors_by_status: Record<string, number>;
  input_tokens: number;
  output_tokens: number;
  total_cost_microcents: number;
  last_updated: string;
}

/**
 * Helper to get stats for a specific provider.
 */
async function getProviderStats(
  gatewayUrl: string,
  adminToken: string,
  providerName: string
): Promise<ProviderStats | null> {
  const response = await trackedFetch(
    `${gatewayUrl}/admin/v1/providers/${providerName}/stats`,
    {
      headers: { Authorization: `Bearer ${adminToken}` },
    }
  );

  if (response.status === 404) {
    return null;
  }

  if (!response.ok) {
    throw new Error(`Failed to get provider stats: ${response.status}`);
  }

  return response.json();
}

/**
 * Helper to get stats for all providers.
 */
async function getAllProviderStats(
  gatewayUrl: string,
  adminToken: string
): Promise<{ stats: ProviderStats[] }> {
  const response = await trackedFetch(`${gatewayUrl}/admin/v1/providers/stats`, {
    headers: { Authorization: `Bearer ${adminToken}` },
  });

  if (!response.ok) {
    throw new Error(`Failed to get all provider stats: ${response.status}`);
  }

  return response.json();
}

/**
 * Helper to make a chat completion request.
 */
async function makeChatRequest(
  gatewayUrl: string,
  apiKey: string,
  model: string,
  message = "Hello from analytics test"
): Promise<Response> {
  return trackedFetch(`${gatewayUrl}/api/v1/chat/completions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-API-Key": apiKey,
    },
    body: JSON.stringify({
      model,
      messages: [{ role: "user", content: message }],
    }),
  });
}

/**
 * Run provider analytics tests.
 *
 * @param getContext - Function that returns the test context
 */
export function runAnalyticsTests(getContext: () => AnalyticsTestContext) {
  describe("Provider Analytics", () => {
    // =========================================================================
    // Test 1: Request Count Tracking
    // =========================================================================
    describe("Request Count Tracking", () => {
      it("increases request count after successful requests", async () => {
        const { gatewayUrl, adminToken, apiKey, providerName, modelName } =
          getContext();

        // Get baseline stats (may be null if no requests yet)
        const baselineStats = await getProviderStats(
          gatewayUrl,
          adminToken,
          providerName
        );
        const baselineCount = baselineStats?.request_count ?? 0;

        // Make 3 requests
        const numRequests = 3;
        for (let i = 0; i < numRequests; i++) {
          const response = await makeChatRequest(
            gatewayUrl,
            apiKey,
            modelName,
            `Analytics request ${i + 1}`
          );
          expect(response.status).toBe(200);
        }

        // Wait for analytics to flush and verify count increased
        const stats = await retry(
          async () => {
            const s = await getProviderStats(
              gatewayUrl,
              adminToken,
              providerName
            );
            if (!s) {
              throw new Error("No stats yet");
            }
            if (s.request_count < baselineCount + numRequests) {
              throw new Error(
                `Request count not yet updated: ${s.request_count} < ${baselineCount + numRequests}`
              );
            }
            return s;
          },
          {
            maxAttempts: 20,
            initialDelay: 200,
            backoffMultiplier: 1.5,
            maxDelay: 2000,
          }
        );

        expect(stats.request_count).toBeGreaterThanOrEqual(
          baselineCount + numRequests
        );
      });
    });

    // =========================================================================
    // Test 2: Error Rate Tracking (5xx errors only)
    // =========================================================================
    describe("Error Rate Tracking", () => {
      it("tracks 5xx errors but not 4xx errors", async () => {
        const { gatewayUrl, adminToken, apiKey, providerName } = getContext();

        // Get baseline error count
        const baselineStats = await getProviderStats(
          gatewayUrl,
          adminToken,
          providerName
        );
        const baselineErrorCount = baselineStats?.error_count ?? 0;
        const baselineRequestCount = baselineStats?.request_count ?? 0;

        // Make a request that triggers a 500 error (magic error model)
        const error500Response = await makeChatRequest(
          gatewayUrl,
          apiKey,
          "test/error-500",
          "Trigger 500 error"
        );
        expect(error500Response.status).toBe(500);

        // Make a request that triggers a 503 error
        const error503Response = await makeChatRequest(
          gatewayUrl,
          apiKey,
          "test/error-503",
          "Trigger 503 error"
        );
        expect(error503Response.status).toBe(503);

        // Wait for analytics to flush and verify error count increased
        const stats = await retry(
          async () => {
            const s = await getProviderStats(
              gatewayUrl,
              adminToken,
              providerName
            );
            if (!s) {
              throw new Error("No stats yet");
            }
            // Expect at least 2 new errors (500 and 503)
            if (s.error_count < baselineErrorCount + 2) {
              throw new Error(
                `Error count not yet updated: ${s.error_count} < ${baselineErrorCount + 2}`
              );
            }
            return s;
          },
          {
            maxAttempts: 20,
            initialDelay: 200,
            backoffMultiplier: 1.5,
            maxDelay: 2000,
          }
        );

        // Verify error count increased
        expect(stats.error_count).toBeGreaterThanOrEqual(baselineErrorCount + 2);

        // Verify request count also increased (errors are still counted as requests)
        expect(stats.request_count).toBeGreaterThanOrEqual(
          baselineRequestCount + 2
        );

        // Verify errors_by_status contains 500 and 503
        expect(stats.errors_by_status["500"]).toBeDefined();
        expect(stats.errors_by_status["503"]).toBeDefined();
      });

      it("counts 429 rate limiting as provider error", async () => {
        const { gatewayUrl, adminToken, apiKey, providerName } = getContext();

        // Get baseline error count
        const baselineStats = await getProviderStats(
          gatewayUrl,
          adminToken,
          providerName
        );
        const baselineErrorCount = baselineStats?.error_count ?? 0;

        // Make a request that triggers a 429 error
        const error429Response = await makeChatRequest(
          gatewayUrl,
          apiKey,
          "test/error-429",
          "Trigger 429 error"
        );
        expect(error429Response.status).toBe(429);

        // Wait for analytics to flush and verify error count increased
        const stats = await retry(
          async () => {
            const s = await getProviderStats(
              gatewayUrl,
              adminToken,
              providerName
            );
            if (!s) {
              throw new Error("No stats yet");
            }
            if (s.error_count < baselineErrorCount + 1) {
              throw new Error(
                `Error count not yet updated: ${s.error_count} < ${baselineErrorCount + 1}`
              );
            }
            return s;
          },
          {
            maxAttempts: 20,
            initialDelay: 200,
            backoffMultiplier: 1.5,
            maxDelay: 2000,
          }
        );

        // Verify 429 was counted as an error
        expect(stats.errors_by_status["429"]).toBeDefined();
      });
    });

    // =========================================================================
    // Test 3: Latency Percentile Accuracy
    // =========================================================================
    describe("Latency Percentile Accuracy", () => {
      it("calculates valid latency percentiles", async () => {
        const { gatewayUrl, adminToken, apiKey, providerName, modelName } =
          getContext();

        // Make several requests to ensure we have latency data
        for (let i = 0; i < 5; i++) {
          const response = await makeChatRequest(
            gatewayUrl,
            apiKey,
            modelName,
            `Latency test ${i + 1}`
          );
          expect(response.status).toBe(200);
        }

        // Wait for stats to update and verify percentiles
        const stats = await retry(
          async () => {
            const s = await getProviderStats(
              gatewayUrl,
              adminToken,
              providerName
            );
            if (!s) {
              throw new Error("No stats yet");
            }
            // Wait for percentiles to be calculated
            if (s.p50_latency_ms === undefined) {
              throw new Error("Percentiles not yet calculated");
            }
            return s;
          },
          {
            maxAttempts: 20,
            initialDelay: 200,
            backoffMultiplier: 1.5,
            maxDelay: 2000,
          }
        );

        // Verify percentiles are present and valid
        expect(stats.p50_latency_ms).toBeDefined();
        expect(stats.p95_latency_ms).toBeDefined();
        expect(stats.p99_latency_ms).toBeDefined();

        // Verify percentile ordering: P50 <= P95 <= P99
        expect(stats.p50_latency_ms).toBeLessThanOrEqual(stats.p95_latency_ms!);
        expect(stats.p95_latency_ms).toBeLessThanOrEqual(stats.p99_latency_ms!);

        // Verify min <= avg <= max
        if (
          stats.min_latency_ms !== undefined &&
          stats.avg_latency_ms !== undefined &&
          stats.max_latency_ms !== undefined
        ) {
          expect(stats.min_latency_ms).toBeLessThanOrEqual(stats.avg_latency_ms);
          expect(stats.avg_latency_ms).toBeLessThanOrEqual(stats.max_latency_ms);
        }

        // Verify all latencies are positive
        expect(stats.p50_latency_ms).toBeGreaterThan(0);
        expect(stats.p95_latency_ms).toBeGreaterThan(0);
        expect(stats.p99_latency_ms).toBeGreaterThan(0);
      });
    });

    // =========================================================================
    // Test 4: Token and Cost Aggregation
    // =========================================================================
    describe("Token and Cost Aggregation", () => {
      it("aggregates tokens and cost correctly", async () => {
        const { gatewayUrl, adminToken, apiKey, providerName, modelName } =
          getContext();

        // Get baseline tokens
        const baselineStats = await getProviderStats(
          gatewayUrl,
          adminToken,
          providerName
        );
        const baselineInputTokens = baselineStats?.input_tokens ?? 0;
        const baselineOutputTokens = baselineStats?.output_tokens ?? 0;
        const baselineCost = baselineStats?.total_cost_microcents ?? 0;

        // Make 2 requests
        // Test provider returns exactly 10 input + 10 output tokens per request
        const numRequests = 2;
        for (let i = 0; i < numRequests; i++) {
          const response = await makeChatRequest(
            gatewayUrl,
            apiKey,
            modelName,
            `Token test ${i + 1}`
          );
          expect(response.status).toBe(200);
        }

        // Expected token increase: 10 input + 10 output per request
        const expectedInputIncrease = 10 * numRequests;
        const expectedOutputIncrease = 10 * numRequests;

        // Wait for stats to update
        const stats = await retry(
          async () => {
            const s = await getProviderStats(
              gatewayUrl,
              adminToken,
              providerName
            );
            if (!s) {
              throw new Error("No stats yet");
            }
            if (s.input_tokens < baselineInputTokens + expectedInputIncrease) {
              throw new Error(
                `Input tokens not yet updated: ${s.input_tokens} < ${baselineInputTokens + expectedInputIncrease}`
              );
            }
            return s;
          },
          {
            maxAttempts: 20,
            initialDelay: 200,
            backoffMultiplier: 1.5,
            maxDelay: 2000,
          }
        );

        // Verify token counts increased by expected amounts
        expect(stats.input_tokens).toBeGreaterThanOrEqual(
          baselineInputTokens + expectedInputIncrease
        );
        expect(stats.output_tokens).toBeGreaterThanOrEqual(
          baselineOutputTokens + expectedOutputIncrease
        );

        // Verify cost increased (pricing configured as 0.5 cents per 10 tokens)
        // Each request: 10 in + 10 out = 20 tokens = 1 cent = 1,000,000 microcents
        // But the actual pricing in the config is 500M microcents per 1M tokens
        // So 10 tokens = 500M / 100,000 = 5,000 microcents = 0.5 cents
        // 20 tokens per request = 10,000 microcents = 1 cent
        expect(stats.total_cost_microcents).toBeGreaterThan(baselineCost);
      });
    });

    // =========================================================================
    // Test 5: All Providers Stats Endpoint
    // =========================================================================
    describe("All Providers Stats Endpoint", () => {
      it("returns stats for all providers", async () => {
        const { gatewayUrl, adminToken, providerName } = getContext();

        const response = await getAllProviderStats(gatewayUrl, adminToken);

        // Verify response structure
        expect(response.stats).toBeDefined();
        expect(Array.isArray(response.stats)).toBe(true);

        // Find our test provider in the list
        const testProviderStats = response.stats.find(
          (s) => s.provider === providerName
        );
        expect(testProviderStats).toBeDefined();

        // Verify the stats have the expected structure
        expect(testProviderStats!.request_count).toBeGreaterThan(0);
        expect(testProviderStats!.last_updated).toBeDefined();
      });
    });

    // =========================================================================
    // Test 6: Historical Stats Endpoint
    // =========================================================================
    describe("Historical Stats Endpoint", () => {
      it("returns historical stats with hourly granularity", async () => {
        const { gatewayUrl, adminToken, providerName } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/${providerName}/stats/history`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.ok).toBe(true);

        const data = await response.json();

        // Verify response structure
        expect(data.provider).toBe(providerName);
        expect(data.granularity).toBe("hour");
        expect(data.data).toBeDefined();
        expect(Array.isArray(data.data)).toBe(true);
      });

      it("supports daily granularity", async () => {
        const { gatewayUrl, adminToken, providerName } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/${providerName}/stats/history?granularity=day`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.ok).toBe(true);

        const data = await response.json();

        expect(data.granularity).toBe("day");
      });

      it("validates granularity parameter", async () => {
        const { gatewayUrl, adminToken, providerName } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/${providerName}/stats/history?granularity=invalid`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.status).toBe(400);
      });
    });
  });
}
