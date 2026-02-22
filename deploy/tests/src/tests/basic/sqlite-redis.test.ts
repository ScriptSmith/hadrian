/**
 * SQLite + Redis Deployment Tests
 *
 * Tests the gateway with SQLite database and Redis cache configuration.
 * Migrated from deploy/test-e2e.sh test_sqlite_redis() function.
 */
import { describe, beforeAll, afterAll } from "vitest";
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

describe("SQLite + Redis Deployment", () => {
  let env: StartedComposeEnvironment;
  let gatewayUrl: string;
  let client: Client;
  const testName = "sqlite-redis";

  // Helper to create tracked clients with API key auth
  const apiKeyClient = (apiKey: string) =>
    createTrackedClient(
      createConfig({
        baseUrl: gatewayUrl,
        headers: { Authorization: `Bearer ${apiKey}` },
      })
    );

  beforeAll(async () => {
    // Port allocation for parallel test execution - each test file uses unique host ports
    // to avoid conflicts. Testcontainers still uses getMappedPort() for the actual URL.
    const gatewayPort = 8081;
    const redisPort = 6380;

    env = await startComposeEnvironment({
      projectName: "hadrian-e2e-sqlite-redis",
      composeFile: "docker-compose.sqlite-redis.yml",
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
});
