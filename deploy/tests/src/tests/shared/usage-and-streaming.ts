/**
 * Usage Tracking and Streaming Tests
 *
 * Tests usage tracking, streaming (SSE format), budget enforcement, and API key scoping.
 * Migrated from test_usage_and_streaming() in deploy/test-e2e.sh.
 *
 * Test scenarios (matching bash script exactly):
 *   1. Non-streaming chat completion with usage field
 *   2. Streaming chat (SSE format: `data:`, `[DONE]`, `delta`)
 *   3. Responses API non-streaming
 *   4. Responses API streaming
 *   5. Usage metrics from admin API
 *   6. Organization usage endpoint
 *   7. Budget enforcement (402 Payment Required)
 *   8. User-scoped API key works
 *
 * Pricing config: Test provider costs exactly 1 cent per request
 * (10 in + 10 out tokens at 0.5 cents/10 tokens)
 */
import { describe, it, expect } from "vitest";
import { trackedFetch } from "../../utils/tracked-fetch";
import { retry } from "../../utils/retry";

/**
 * Context for usage and streaming tests.
 */
export interface UsageAndStreamingContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /** Service account token for admin API access */
  adminToken: string;
  /** API keys created during setup */
  apiKeys: {
    /** Org-scoped API key (no budget limit) */
    org: { id: string; key: string };
    /** Budget-limited API key (1 cent daily) */
    budget: { id: string; key: string };
    /** User-scoped API key (for phd_bob) */
    user: { id: string; key: string };
  };
}

/**
 * Helper to get UTC date in YYYY-MM-DD format.
 */
function getUtcDate(): string {
  const now = new Date();
  return now.toISOString().split("T")[0];
}

/**
 * Run usage tracking and streaming tests.
 * Tests match test_usage_and_streaming() from bash script exactly.
 *
 * @param getContext - Function that returns the test context
 */
export function runUsageAndStreamingTests(
  getContext: () => UsageAndStreamingContext
) {
  describe("Usage Tracking and Streaming", () => {
    // =========================================================================
    // Test 1: Non-streaming chat completion
    // =========================================================================
    describe("Non-streaming Chat Completion", () => {
      it("returns content and usage field", async () => {
        const { gatewayUrl, apiKeys } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKeys.org.key,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello from usage test" }],
            }),
          }
        );

        expect(response.status).toBe(200);

        const data = await response.json();

        // Verify content is present
        expect(data.choices).toBeDefined();
        expect(data.choices.length).toBeGreaterThan(0);
        expect(data.choices[0].message.content).toBeDefined();

        // Verify usage field is present
        expect(data.usage).toBeDefined();
        expect(data.usage.prompt_tokens).toBeDefined();
        expect(data.usage.completion_tokens).toBeDefined();
        expect(data.usage.total_tokens).toBeDefined();
      });
    });

    // =========================================================================
    // Test 2: Streaming chat completion (SSE format verification)
    // =========================================================================
    describe("Streaming Chat Completion", () => {
      it("returns SSE format with data: prefix, [DONE] marker, and delta chunks", async () => {
        const { gatewayUrl, apiKeys } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKeys.org.key,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello streaming" }],
              stream: true,
            }),
          }
        );

        expect(response.status).toBe(200);

        const text = await response.text();

        // Verify SSE format: should contain "data:" lines
        expect(text).toMatch(/^data:/m);

        // Verify stream contains [DONE] marker
        expect(text).toContain("[DONE]");

        // Verify stream contains delta content
        expect(text).toContain('"delta"');
      });
    });

    // =========================================================================
    // Test 3: Responses API (non-streaming)
    // =========================================================================
    describe("Responses API Non-streaming", () => {
      it("returns completed status and usage field", async () => {
        const { gatewayUrl, apiKeys } = getContext();

        const response = await trackedFetch(`${gatewayUrl}/api/v1/responses`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-API-Key": apiKeys.org.key,
          },
          body: JSON.stringify({
            model: "test/test-model",
            input: "Hello from responses API test",
          }),
        });

        expect(response.status).toBe(200);

        const data = await response.json();

        // Verify status is completed
        expect(data.status).toBe("completed");

        // Verify usage field is present
        expect(data.usage).toBeDefined();
      });
    });

    // =========================================================================
    // Test 4: Responses API (streaming)
    // =========================================================================
    describe("Responses API Streaming", () => {
      it("returns SSE format with response.completed event", async () => {
        const { gatewayUrl, apiKeys } = getContext();

        const response = await trackedFetch(`${gatewayUrl}/api/v1/responses`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-API-Key": apiKeys.org.key,
          },
          body: JSON.stringify({
            model: "test/test-model",
            input: "Hello streaming responses",
            stream: true,
          }),
        });

        expect(response.status).toBe(200);

        const text = await response.text();

        // Verify SSE format
        expect(text).toMatch(/^data:/m);

        // Responses API uses different event types
        expect(text).toContain('"type":"response.completed"');
      });
    });

    // =========================================================================
    // Test 5: Usage metrics from admin API
    // =========================================================================
    describe("Usage Metrics from Admin API", () => {
      it("returns request count for API key", async () => {
        const { gatewayUrl, adminToken, apiKeys } = getContext();

        const today = getUtcDate();

        // Use retry to wait for async usage tracking to flush to database
        // (university config uses 100ms flush interval, poll until data is available)
        const data = await retry(
          async () => {
            const response = await trackedFetch(
              `${gatewayUrl}/admin/v1/api-keys/${apiKeys.org.id}/usage?start_date=${today}&end_date=${today}`,
              {
                headers: { Authorization: `Bearer ${adminToken}` },
              }
            );

            expect(response.status).toBe(200);

            const result = await response.json();

            // Retry if request_count is 0 (usage not yet flushed)
            if (!result.request_count || result.request_count === 0) {
              throw new Error("Usage not yet flushed");
            }

            return result;
          },
          {
            maxAttempts: 10,
            initialDelay: 500,
            backoffMultiplier: 1.5,
            maxDelay: 2000,
          }
        );

        // Verify request_count is present and > 0
        expect(data.request_count).toBeDefined();
        expect(data.request_count).toBeGreaterThan(0);
      });

      it("returns usage by model (optional - may not have data)", async () => {
        const { gatewayUrl, adminToken, apiKeys } = getContext();

        const today = getUtcDate();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys/${apiKeys.org.id}/usage/by-model?start_date=${today}&end_date=${today}`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        // This endpoint should at least return successfully
        expect(response.status).toBe(200);
      });
    });

    // =========================================================================
    // Test 6: Organization usage endpoint
    // =========================================================================
    describe("Organization Usage Endpoint", () => {
      it("returns request count for organization", async () => {
        const { gatewayUrl, adminToken } = getContext();

        const today = getUtcDate();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/usage?start_date=${today}&end_date=${today}`,
          {
            headers: { Authorization: `Bearer ${adminToken}` },
          }
        );

        expect(response.status).toBe(200);

        const data = await response.json();

        // Verify request_count is present
        expect(data.request_count).toBeDefined();
      });
    });

    // =========================================================================
    // Test 7: Budget enforcement - exhaust budget then verify 402
    // =========================================================================
    describe("Budget Enforcement", () => {
      it("returns 200 for first request then 402 for second (budget exhausted)", async () => {
        const { gatewayUrl, apiKeys } = getContext();

        // Budget key has 1 cent daily limit. Pricing is configured so each request costs 1 cent.
        // First request should succeed and exhaust the budget, second should fail with 402.

        // First request - should succeed and use the entire 1 cent budget
        const response1 = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKeys.budget.key,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Budget test 1" }],
            }),
          }
        );

        expect(response1.status).toBe(200);

        // Second request - should fail with 402 (budget exhausted)
        const response2 = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKeys.budget.key,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Budget test 2" }],
            }),
          }
        );

        expect(response2.status).toBe(402);
      });
    });

    // =========================================================================
    // Test 8: User-scoped API key works
    // =========================================================================
    describe("User-scoped API Key", () => {
      it("successfully completes chat request", async () => {
        const { gatewayUrl, apiKeys } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKeys.user.key,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello from user key" }],
            }),
          }
        );

        expect(response.status).toBe(200);

        const data = await response.json();

        // Verify content is present
        expect(data.choices).toBeDefined();
        expect(data.choices.length).toBeGreaterThan(0);
        expect(data.choices[0].message.content).toBeDefined();
      });
    });
  });
}
