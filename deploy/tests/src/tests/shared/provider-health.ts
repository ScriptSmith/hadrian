/**
 * Provider Health Dashboard Tests
 *
 * Tests the provider health monitoring, circuit breaker status, and
 * Prometheus-based statistics endpoints.
 *
 * Endpoints tested:
 * - GET /admin/v1/providers/health - List all provider health statuses
 * - GET /admin/v1/providers/{name}/health - Get single provider health
 * - GET /admin/v1/providers/circuit-breakers - List all circuit breakers
 * - GET /admin/v1/providers/{name}/circuit-breaker - Get single circuit breaker
 * - GET /admin/v1/providers/stats - Get aggregated stats from Prometheus
 * - GET /admin/v1/providers/{name}/stats/history - Get historical time series
 */
import { describe, it, expect } from "vitest";
import { trackedFetch } from "../../utils/tracked-fetch";
import { retry } from "../../utils/retry";

/**
 * Context for provider health tests.
 */
export interface ProviderHealthContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /** Admin token for admin API access */
  adminToken: string;
  /** API key for making requests */
  apiKey: string;
  /** Provider name to test (e.g., "test") */
  providerName: string;
  /** Whether Prometheus is configured */
  hasPrometheus: boolean;
}

/**
 * Provider health status from the admin API.
 */
interface ProviderHealthState {
  provider: string;
  status: string; // "healthy", "unhealthy", "unknown"
  latency_ms: number;
  error?: string;
  status_code?: number;
  last_check: string;
  consecutive_successes: number;
  consecutive_failures: number;
}

/**
 * Circuit breaker status from the admin API.
 */
interface CircuitBreakerStatus {
  provider: string;
  state: string;
  failure_count: number;
}

/**
 * Provider stats from the admin API.
 */
interface ProviderStats {
  provider: string;
  p50_latency_ms?: number;
  p95_latency_ms?: number;
  p99_latency_ms?: number;
  avg_latency_ms?: number;
  request_count: number;
  error_count: number;
  errors_by_status: Record<string, number>;
  input_tokens: number;
  output_tokens: number;
  total_cost_microcents: number;
  last_updated: string;
}

/**
 * Helper to make a chat completion request.
 */
async function makeChatRequest(
  gatewayUrl: string,
  apiKey: string,
  model: string,
  message = "Hello from provider health test"
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
 * Run provider health tests.
 *
 * @param getContext - Function that returns the test context
 */
export function runProviderHealthTests(
  getContext: () => ProviderHealthContext
) {
  describe("Provider Health Endpoints", () => {
    // =========================================================================
    // Health Status Tests
    // =========================================================================
    describe("Health Status", () => {
      it("lists all provider health statuses", async () => {
        const { gatewayUrl, adminToken } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/health`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.ok).toBe(true);
        const data = (await response.json()) as {
          providers: ProviderHealthState[];
        };

        expect(data.providers).toBeDefined();
        expect(Array.isArray(data.providers)).toBe(true);
      });

      it("gets health status for a specific provider", async () => {
        const { gatewayUrl, adminToken, providerName } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/${providerName}/health`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.ok).toBe(true);
        const data = (await response.json()) as ProviderHealthState;

        expect(data.provider).toBe(providerName);
        expect(["healthy", "unhealthy", "unknown"]).toContain(data.status);
        expect(typeof data.latency_ms).toBe("number");
        expect(typeof data.consecutive_successes).toBe("number");
        expect(typeof data.consecutive_failures).toBe("number");
      });

      it("returns 404 for unknown provider health", async () => {
        const { gatewayUrl, adminToken } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/nonexistent-provider/health`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.status).toBe(404);
      });
    });

    // =========================================================================
    // Circuit Breaker Tests
    // =========================================================================
    describe("Circuit Breaker Status", () => {
      it("lists all circuit breaker statuses", async () => {
        const { gatewayUrl, adminToken } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/circuit-breakers`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.ok).toBe(true);
        const data = (await response.json()) as {
          circuit_breakers: CircuitBreakerStatus[];
        };

        expect(data.circuit_breakers).toBeDefined();
        expect(Array.isArray(data.circuit_breakers)).toBe(true);
      });

      it("gets circuit breaker status for a specific provider", async () => {
        const { gatewayUrl, adminToken, providerName } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/${providerName}/circuit-breaker`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.ok).toBe(true);
        const data = (await response.json()) as CircuitBreakerStatus;

        expect(data.provider).toBe(providerName);
        expect(["closed", "open", "half_open"]).toContain(data.state);
        expect(typeof data.failure_count).toBe("number");
      });

      it("returns 404 for unknown provider circuit breaker", async () => {
        const { gatewayUrl, adminToken } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/nonexistent-provider/circuit-breaker`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.status).toBe(404);
      });
    });

    // =========================================================================
    // Circuit Breaker State Transition Tests
    // =========================================================================
    // NOTE: Circuit breaker state transition tests are skipped when using the test
    // provider because TestProvider doesn't integrate with the circuit breaker registry.
    // These tests would work with real providers (OpenAI, Anthropic, etc.) that
    // properly register their circuit breakers.
    //
    // To test circuit breaker state transitions:
    // 1. Configure a real provider with circuit_breaker.enabled = true
    // 2. Use a model that can return 5xx errors
    // 3. The circuit breaker will track failures and open after threshold

    // =========================================================================
    // Provider Stats Tests (Prometheus-based)
    // =========================================================================
    describe("Provider Stats (Prometheus)", () => {
      it("returns stats for all providers", async () => {
        const { gatewayUrl, adminToken, apiKey, hasPrometheus } = getContext();

        // Make some requests to generate metrics
        for (let i = 0; i < 3; i++) {
          await makeChatRequest(gatewayUrl, apiKey, "test/test-model");
        }

        // Wait for Prometheus to scrape (scrape_interval: 5s) and stats to appear
        const data = await retry(
          async () => {
            const response = await trackedFetch(
              `${gatewayUrl}/admin/v1/providers/stats`,
              {
                headers: { Authorization: `Bearer ${adminToken}` },
              }
            );

            if (!response.ok) {
              throw new Error(`Stats endpoint failed: ${response.status}`);
            }

            const json = (await response.json()) as { stats: ProviderStats[] };

            // In Prometheus mode, we need to wait for scrape
            if (hasPrometheus && json.stats.length === 0) {
              throw new Error("Stats not yet available");
            }

            return json;
          },
          {
            maxAttempts: 20,
            initialDelay: 1000,
            backoffMultiplier: 1.3,
            maxDelay: 5000,
          }
        );

        expect(data.stats).toBeDefined();
        expect(Array.isArray(data.stats)).toBe(true);
      });

      it("returns stats for a specific provider", async () => {
        const { gatewayUrl, adminToken, apiKey, providerName, hasPrometheus } =
          getContext();

        // Make some requests to generate metrics
        await makeChatRequest(gatewayUrl, apiKey, "test/test-model");

        // Wait for stats to be available
        const stats = await retry(
          async () => {
            const response = await trackedFetch(
              `${gatewayUrl}/admin/v1/providers/${providerName}/stats`,
              {
                headers: { Authorization: `Bearer ${adminToken}` },
              }
            );

            // Stats might return 404 if no data yet
            if (response.status === 404 && hasPrometheus) {
              throw new Error("Stats not yet available");
            }

            if (!response.ok) {
              throw new Error(`Stats endpoint failed: ${response.status}`);
            }

            return (await response.json()) as ProviderStats;
          },
          {
            maxAttempts: 20,
            initialDelay: 1000,
            backoffMultiplier: 1.3,
            maxDelay: 5000,
          }
        );

        expect(stats.provider).toBe(providerName);
        expect(stats.request_count).toBeGreaterThan(0);
        expect(typeof stats.error_count).toBe("number");
        expect(typeof stats.input_tokens).toBe("number");
        expect(typeof stats.output_tokens).toBe("number");
      });
    });

    // =========================================================================
    // Historical Stats Tests
    // =========================================================================
    describe("Historical Stats", () => {
      it("returns historical stats with hourly granularity", async () => {
        const { gatewayUrl, adminToken, providerName, hasPrometheus } =
          getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/providers/${providerName}/stats/history`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.ok).toBe(true);
        const data = await response.json();

        expect(data.provider).toBe(providerName);
        expect(data.granularity).toBe("hour");
        expect(Array.isArray(data.data)).toBe(true);

        // In non-Prometheus mode, data array will be empty
        if (!hasPrometheus) {
          expect(data.data.length).toBe(0);
        }
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
