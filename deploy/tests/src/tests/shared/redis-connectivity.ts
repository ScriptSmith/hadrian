/**
 * Redis connectivity tests
 *
 * Tests that verify Redis cache is properly connected and healthy.
 * Used by deployments with Redis: sqlite-redis, postgres, dlq, observability,
 * postgres-ha, redis-cluster, traefik, university, saml.
 */
import { describe, it, expect } from "vitest";
import type { Client } from "../../client/client";
import type { ExecResult } from "../../fixtures/compose";
import { healthCheck } from "../../client";

export interface RedisConnectivityContext {
  url: string;
  client: Client;
  /** Optional: execute commands in a container (for direct Redis verification) */
  execInService?: (
    serviceName: string,
    command: string[]
  ) => Promise<ExecResult>;
}

/**
 * Run Redis connectivity tests via the health endpoint.
 * @param getContext - Function that returns the test context. Called lazily to ensure
 *                     the context is available after beforeAll setup completes.
 */
export function runRedisConnectivityTests(
  getContext: () => RedisConnectivityContext
) {
  describe("Redis Connectivity", () => {
    it("reports cache subsystem in health endpoint", async () => {
      const { client } = getContext();
      const response = await healthCheck({ client });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      expect(response.data?.status).toBe("healthy");
      // Cache subsystem should be present and healthy when Redis is configured
      expect(response.data?.subsystems.cache).toBeDefined();
      expect(response.data?.subsystems.cache?.healthy).toBe(true);
    });

    it("cache subsystem reports latency", async () => {
      const { client } = getContext();
      const response = await healthCheck({ client });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      // Latency should be a reasonable number (less than 1 second for local Redis)
      if (response.data?.subsystems.cache?.latency_ms !== undefined) {
        expect(response.data.subsystems.cache.latency_ms).toBeGreaterThanOrEqual(
          0
        );
        expect(response.data.subsystems.cache.latency_ms).toBeLessThan(1000);
      }
    });

    it("verifies Redis has active connections", async () => {
      const { execInService } = getContext();

      // Skip this test if execInService is not available
      if (!execInService) {
        return;
      }

      // Execute redis-cli info clients in the Redis container
      const result = await execInService("redis", [
        "redis-cli",
        "info",
        "clients",
      ]);

      // Check that Redis has connected clients (gateway should be connected)
      expect(result.output).toContain("connected_clients");
      // Parse connected_clients value - should be at least 1 (the gateway)
      const match = result.output.match(/connected_clients:(\d+)/);
      if (match) {
        const connectedClients = parseInt(match[1], 10);
        expect(connectedClients).toBeGreaterThanOrEqual(1);
      }
    });
  });
}
