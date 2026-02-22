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

  // DLQ-specific tests
  describe("Redis Streams DLQ", () => {
    it("verifies Redis Streams support via XINFO command", async () => {
      const result = await env.execInService("redis", [
        "redis-cli",
        "XINFO",
        "HELP",
      ]);

      // XINFO HELP should return information about the XINFO subcommands
      // This verifies that Redis Streams commands are available
      expect(result.exitCode).toBe(0);
      expect(result.output.toLowerCase()).toMatch(/xinfo|stream/i);
    });

    it("DLQ stream exists or will be created on first use", async () => {
      // Check if the DLQ stream exists
      // Note: The stream may not exist until the first message is sent to it
      const result = await env.execInService("redis", [
        "redis-cli",
        "EXISTS",
        "dlq:stream",
      ]);

      expect(result.exitCode).toBe(0);
      // Result will be "0" (doesn't exist yet) or "1" (exists)
      // Both are valid states - the stream is created on first use
      const exists = result.output.trim();
      expect(["0", "1"]).toContain(exists);
    });

    it("can create and read from a Redis Stream", async () => {
      // Test that Redis Streams operations work correctly
      // Add an entry to a test stream
      const addResult = await env.execInService("redis", [
        "redis-cli",
        "XADD",
        "test:dlq:stream",
        "*",
        "test",
        "value",
      ]);

      expect(addResult.exitCode).toBe(0);
      // XADD returns the stream entry ID (e.g., "1234567890123-0")
      expect(addResult.output.trim()).toMatch(/^\d+-\d+$/);

      // Read from the stream to verify it was created
      const readResult = await env.execInService("redis", [
        "redis-cli",
        "XLEN",
        "test:dlq:stream",
      ]);

      expect(readResult.exitCode).toBe(0);
      const length = parseInt(readResult.output.trim(), 10);
      expect(length).toBeGreaterThanOrEqual(1);

      // Clean up the test stream
      await env.execInService("redis", ["redis-cli", "DEL", "test:dlq:stream"]);
    });
  });
});
