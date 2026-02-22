/**
 * SQLite Deployment Tests
 *
 * Tests the gateway with SQLite database configuration.
 * Migrated from deploy/test-e2e.sh test_sqlite() function.
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

describe("SQLite Deployment", () => {
  let env: StartedComposeEnvironment;
  let gatewayUrl: string;
  let client: Client;
  const testName = "sqlite";

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
    const gatewayPort = 8080;

    env = await startComposeEnvironment({
      projectName: "hadrian-e2e-sqlite",
      composeFile: "docker-compose.sqlite.yml",
      waitForServices: {
        gateway: { port: 8080, path: "/health" },
      },
      env: {
        GATEWAY_PORT: String(gatewayPort),
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
});
