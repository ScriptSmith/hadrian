/**
 * Provider Health Dashboard Deployment Tests
 *
 * Tests the gateway with provider health monitoring, circuit breaker,
 * and Prometheus integration for metrics-based statistics.
 *
 * Services:
 * - Gateway with health checks and circuit breaker enabled
 * - Prometheus for metrics aggregation
 *
 * Tests cover:
 * - Provider health status endpoints
 * - Circuit breaker status and state transitions
 * - Prometheus-based statistics
 * - WebSocket event streaming for real-time updates
 */
import { describe, beforeAll, afterAll } from "vitest";
import {
  startComposeEnvironment,
  createTrackedClient,
  type StartedComposeEnvironment,
} from "../../fixtures";
import { createConfig } from "../../client/client";
import type { Client } from "../../client/client";
import { organizationCreate, apiKeyCreate } from "../../client";
import { runHealthCheckTests } from "../shared/health-checks";
import { runProviderHealthTests } from "../shared/provider-health";
import { runWebSocketEventTests } from "../shared/websocket-events";

describe("Provider Health Dashboard Deployment", () => {
  let env: StartedComposeEnvironment;
  let gatewayUrl: string;
  let client: Client;
  let apiKey: string;
  const testName = "provider-health";
  const providerName = "test";

  beforeAll(async () => {
    // Port allocation for parallel test execution
    // Using unique ports to avoid conflicts with other test files
    // Note: 8090 is used by saml-authentik.test.ts
    const gatewayPort = 8091;
    const prometheusPort = 9092;

    env = await startComposeEnvironment({
      projectName: "hadrian-e2e-provider-health",
      composeFile: "docker-compose.provider-health.yml",
      waitForServices: {
        gateway: { port: 8080, path: "/health" },
        prometheus: { port: 9090, path: "/-/healthy" },
      },
      env: {
        GATEWAY_PORT: String(gatewayPort),
        PROMETHEUS_PORT: String(prometheusPort),
      },
      // Longer startup timeout for Prometheus to be ready
      startupTimeout: 120000,
    });

    gatewayUrl = env.getServiceUrl("gateway", 8080);
    client = createTrackedClient(createConfig({ baseUrl: gatewayUrl }));

    // Create an organization for testing
    const orgResponse = await organizationCreate({
      client,
      body: {
        slug: `${testName}-org`,
        name: `Provider Health Test Organization`,
      },
    });
    const orgId = orgResponse.data!.id;

    // Create an API key for making requests
    const keyResponse = await apiKeyCreate({
      client,
      body: {
        name: "Provider Health Test Key",
        owner: {
          type: "organization",
          org_id: orgId,
        },
      },
    });
    apiKey = keyResponse.data!.key!;
  });

  afterAll(async () => {
    await env?.stop();
  });

  // Run shared test suites
  runHealthCheckTests(() => ({ url: gatewayUrl, client }));

  runProviderHealthTests(() => ({
    gatewayUrl,
    adminToken: "", // Admin endpoints don't require token when auth is not configured
    apiKey,
    providerName,
    hasPrometheus: true,
  }));

  runWebSocketEventTests(() => ({
    gatewayUrl,
    adminToken: "",
    apiKey,
    providerName,
  }));
});
