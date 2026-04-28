/**
 * DLQ (Dead Letter Queue) Deployment Tests
 *
 * Tests the gateway with SQLite database, Redis cache, and Redis Streams DLQ configuration.
 * Migrated from deploy/test-e2e.sh test_dlq() function.
 */
import { describe, beforeAll, afterAll, it, expect } from "vitest";
import {
  startComposeEnvironment,
  createTrackedClient,
  type StartedComposeEnvironment,
} from "../../fixtures";
import { createConfig } from "../../client/client";
import type { Client } from "../../client/client";
import { dlqList, dlqPurge, dlqStats } from "../../client";
import { runHealthCheckTests } from "../shared/health-checks";
import { runAdminApiCrudTests } from "../shared/admin-api-crud";
import { runChatCompletionsTests } from "../shared/chat-completions";
import { runRedisConnectivityTests } from "../shared/redis-connectivity";

describe("DLQ (Dead Letter Queue) Deployment", () => {
  let env: StartedComposeEnvironment;
  let gatewayUrl: string;
  let client: Client;
  const testName = "dlq";

  // Helper to create tracked clients with API key auth
  const apiKeyClient = (apiKey: string) =>
    createTrackedClient(
      createConfig({
        baseUrl: gatewayUrl,
        headers: { Authorization: `Bearer ${apiKey}` },
      })
    );

  beforeAll(async () => {
    // Port allocation for parallel test execution (see DEPLOY_TEST_REWRITE.md)
    const gatewayPort = 8083;
    const redisPort = 6382;

    env = await startComposeEnvironment({
      projectName: "hadrian-e2e-dlq",
      composeFile: "docker-compose.dlq.yml",
      waitForServices: {
        gateway: { port: 8080, path: "/health" },
        // Redis health is verified via gateway readiness (gateway depends on redis)
      },
      env: {
        GATEWAY_PORT: String(gatewayPort),
        REDIS_PORT: String(redisPort),
      },
    });
    gatewayUrl = env.getServiceUrl("gateway", 8080);
    // Use createTrackedClient for API coverage tracking
    client = createTrackedClient(createConfig({ baseUrl: gatewayUrl }));
  });

  afterAll(async () => {
    await env?.stop();
  });

  // Run shared test suites
  // Pass functions that return context to ensure values are available after beforeAll
  runHealthCheckTests(() => ({ url: gatewayUrl, client }));
  runAdminApiCrudTests(() => ({ url: gatewayUrl, client, testName }));
  runChatCompletionsTests(() => ({
    url: gatewayUrl,
    client,
    apiKeyClient,
    testName,
  }));
  runRedisConnectivityTests(() => ({
    url: gatewayUrl,
    client,
    execInService: env.execInService,
  }));

  // DLQ-specific tests — exercise Hadrian's DLQ admin endpoints directly so we
  // catch wiring regressions (route → service → backend) rather than just
  // proving Redis itself supports streams (which is a Redis property, not a
  // Hadrian one).
  describe("Hadrian DLQ admin API", () => {
    it("returns stats with the documented shape", async () => {
      const response = await dlqStats({ client });

      expect(response.response.status).toBe(200);
      const data = response.data;
      if (!data) throw new Error("dlqStats returned no body");

      expect(typeof data.total_entries).toBe("number");
      expect(data.total_entries).toBeGreaterThanOrEqual(0);
      expect(typeof data.is_empty).toBe("boolean");
      expect(data.is_empty).toBe(data.total_entries === 0);
      // by_type / by_retry_count are populated lazily; just assert object shape.
      expect(typeof data.by_type).toBe("object");
      expect(typeof data.by_retry_count).toBe("object");
    });

    it("returns a paginated list", async () => {
      const response = await dlqList({
        client,
        query: { limit: 10 },
      });

      expect(response.response.status).toBe(200);
      const data = response.data;
      if (!data) throw new Error("dlqList returned no body");

      expect(Array.isArray(data.data)).toBe(true);
      expect(data.pagination).toBeDefined();
      expect(typeof data.pagination?.limit).toBe("number");
      expect(typeof data.pagination?.has_more).toBe("boolean");
    });

    it("rejects an invalid pagination cursor with a 400", async () => {
      const response = await dlqList({
        client,
        query: { cursor: "not-a-real-cursor" },
      });

      // The route is documented to return 400 for bad cursors, not 500.
      expect(response.response.status).toBe(400);
    });

    it("purge succeeds (idempotent on an empty queue)", async () => {
      // Purge is idempotent — even on an empty DLQ it should return 200 and
      // a body documenting the result. Catches the route → cache wiring without
      // needing to inject failures from outside the gateway.
      const response = await dlqPurge({ client });

      expect(response.response.status).toBe(200);
      const after = await dlqStats({ client });
      expect(after.response.status).toBe(200);
      expect(after.data?.is_empty).toBe(true);
    });
  });
});
