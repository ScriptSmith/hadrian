/**
 * Redis Cluster Deployment Tests
 *
 * Tests the gateway with a 6-node Redis Cluster (3 masters + 3 replicas):
 * - redis-1, redis-2, redis-3: Initial nodes (become masters or replicas)
 * - redis-4, redis-5, redis-6: Additional nodes (become masters or replicas)
 *
 * The cluster create command with --cluster-replicas 1 automatically assigns
 * roles to nodes (3 masters and 3 replicas for high availability).
 *
 * Migrated from deploy/test-e2e.sh test_redis_cluster() function.
 */
import { describe, beforeAll, afterAll, it, expect } from "vitest";
import {
  startComposeEnvironment,
  createTrackedClient,
  waitForHealthy,
  type StartedComposeEnvironment,
} from "../../fixtures";
import { createConfig } from "../../client/client";
import type { Client } from "../../client/client";
import { runHealthCheckTests } from "../shared/health-checks";
import { runAdminApiCrudTests } from "../shared/admin-api-crud";
import { runChatCompletionsTests } from "../shared/chat-completions";
import { runRedisConnectivityTests } from "../shared/redis-connectivity";

describe("Redis Cluster Deployment (3 Masters + 3 Replicas)", () => {
  let env: StartedComposeEnvironment;
  let gatewayUrl: string;
  let client: Client;
  const testName = "redis-cluster";

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
    const gatewayPort = 8086;

    // Follow the same flow as the original bash test (test-e2e.sh:765-822):
    // 1. Start services
    // 2. Wait for Redis nodes to be healthy
    // 3. Initialize the cluster
    // 4. Wait for gateway
    env = await startComposeEnvironment({
      projectName: "hadrian-e2e-redis-cluster",
      composeFile: "docker-compose.redis-cluster.yml",
      // Don't wait for gateway - it will restart until cluster is ready
      waitForServices: {},
      // Wait for Redis nodes via Docker healthcheck (redis-cli ping)
      serviceWaitStrategies: {
        "redis-1": { type: "healthcheck" },
        "redis-2": { type: "healthcheck" },
        "redis-3": { type: "healthcheck" },
        "redis-4": { type: "healthcheck" },
        "redis-5": { type: "healthcheck" },
        "redis-6": { type: "healthcheck" },
      },
      env: {
        GATEWAY_PORT: String(gatewayPort),
      },
      startupTimeout: 180000,
    });

    // Initialize the cluster (matches bash test line 791-794)
    await env.execInService("redis-1", [
      "redis-cli",
      "--cluster",
      "create",
      "redis-1:6379",
      "redis-2:6379",
      "redis-3:6379",
      "redis-4:6379",
      "redis-5:6379",
      "redis-6:6379",
      "--cluster-replicas",
      "1",
      "--cluster-yes",
    ]);

    // Wait for cluster to stabilize (matches bash test line 795)
    await new Promise((resolve) => setTimeout(resolve, 5000));

    // Verify cluster is healthy
    const clusterInfo = await env.execInService("redis-1", [
      "redis-cli",
      "cluster",
      "info",
    ]);
    expect(clusterInfo.output).toContain("cluster_state:ok");

    // Wait for gateway to become healthy (matches bash test wait_for_gateway)
    gatewayUrl = env.getServiceUrl("gateway", 8080);
    await waitForHealthy(`${gatewayUrl}/health`, {
      maxRetries: 60,
      retryInterval: 2000,
    });

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

  // Redis connectivity tests via health endpoint
  // Now enabled since the gateway supports Redis Cluster mode with proper MOVED/ASK handling
  runRedisConnectivityTests(() => ({
    url: gatewayUrl,
    client,
    // For cluster mode, we check redis-1 since there's no single "redis" service
    execInService: (service, cmd) => {
      // Map "redis" to "redis-1" for cluster deployments
      const actualService = service === "redis" ? "redis-1" : service;
      return env.execInService(actualService, cmd);
    },
  }));

  // Redis Cluster specific tests
  describe("Redis Cluster Architecture", () => {
    it("cluster state is healthy", async () => {
      const result = await env.execInService("redis-1", [
        "redis-cli",
        "cluster",
        "info",
      ]);
      expect(result.exitCode).toBe(0);
      expect(result.output).toContain("cluster_state:ok");
    });

    it("cluster has correct node count (6 nodes)", async () => {
      const result = await env.execInService("redis-1", [
        "redis-cli",
        "cluster",
        "nodes",
      ]);
      expect(result.exitCode).toBe(0);
      // Each line represents a node in the cluster
      const lines = result.output
        .trim()
        .split("\n")
        .filter((line) => line.length > 0);
      expect(lines.length).toBe(6); // 3 masters + 3 replicas
    });

    it("cluster has 3 master nodes", async () => {
      const result = await env.execInService("redis-1", [
        "redis-cli",
        "cluster",
        "nodes",
      ]);
      expect(result.exitCode).toBe(0);
      // Master nodes have "master" in their flags
      const masterCount = result.output
        .split("\n")
        .filter((line) => line.includes("master")).length;
      expect(masterCount).toBe(3);
    });

    it("cluster has 3 replica nodes", async () => {
      const result = await env.execInService("redis-1", [
        "redis-cli",
        "cluster",
        "nodes",
      ]);
      expect(result.exitCode).toBe(0);
      // Replica nodes have "slave" in their flags
      const replicaCount = result.output
        .split("\n")
        .filter((line) => line.includes("slave")).length;
      expect(replicaCount).toBe(3);
    });

    it("cluster has all slots assigned (16384)", async () => {
      const result = await env.execInService("redis-1", [
        "redis-cli",
        "cluster",
        "info",
      ]);
      expect(result.exitCode).toBe(0);
      expect(result.output).toContain("cluster_slots_assigned:16384");
    });

    it("cluster has no slots in fail state", async () => {
      const result = await env.execInService("redis-1", [
        "redis-cli",
        "cluster",
        "info",
      ]);
      expect(result.exitCode).toBe(0);
      expect(result.output).toContain("cluster_slots_fail:0");
    });

    it("all cluster nodes are connected", async () => {
      const result = await env.execInService("redis-1", [
        "redis-cli",
        "cluster",
        "info",
      ]);
      expect(result.exitCode).toBe(0);
      // cluster_known_nodes should be 6
      const match = result.output.match(/cluster_known_nodes:(\d+)/);
      expect(match).not.toBeNull();
      if (match) {
        expect(parseInt(match[1], 10)).toBe(6);
      }
    });
  });

});
