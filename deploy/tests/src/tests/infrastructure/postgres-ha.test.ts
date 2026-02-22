/**
 * PostgreSQL High Availability Deployment Tests
 *
 * Tests the gateway with PostgreSQL primary + read replicas:
 * - postgres-primary: Handles all writes
 * - postgres-replica-1 & postgres-replica-2: Handle read queries
 * - pgbouncer-primary: Connection pooling for writes
 * - pgbouncer-replica: Connection pooling for reads
 * - Redis: Cache layer
 *
 * Migrated from deploy/test-e2e.sh test_postgres_ha() function.
 */
import { describe, beforeAll, afterAll, it, expect } from "vitest";
import {
  startComposeEnvironment,
  createTrackedClient,
  type StartedComposeEnvironment,
} from "../../fixtures";
import { createConfig } from "../../client/client";
import type { Client } from "../../client/client";
import { healthCheck } from "../../client";
import { runHealthCheckTests } from "../shared/health-checks";
import { runAdminApiCrudTests } from "../shared/admin-api-crud";
import { runChatCompletionsTests } from "../shared/chat-completions";
import { runRedisConnectivityTests } from "../shared/redis-connectivity";
import { runPostgresDataTests } from "../shared/postgres-data";

describe("PostgreSQL HA Deployment (Primary + Replicas)", () => {
  let env: StartedComposeEnvironment;
  let gatewayUrl: string;
  let client: Client;
  const testName = "postgres-ha";

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
    const gatewayPort = 8085;
    const pgbouncerPrimaryPort = 6434;
    const pgbouncerReplicaPort = 6435;

    env = await startComposeEnvironment({
      projectName: "hadrian-e2e-postgres-ha",
      composeFile: "docker-compose.postgres-ha.yml",
      waitForServices: {
        gateway: { port: 8080, path: "/health" },
      },
      // Use Docker healthchecks for PostgreSQL and PgBouncer instead of port waiting
      serviceWaitStrategies: {
        "postgres-primary": { type: "healthcheck" },
        "postgres-replica-1": { type: "healthcheck" },
        "postgres-replica-2": { type: "healthcheck" },
        "pgbouncer-primary": { type: "healthcheck" },
        "pgbouncer-replica": { type: "healthcheck" },
        redis: { type: "healthcheck" },
      },
      env: {
        GATEWAY_PORT: String(gatewayPort),
        PGBOUNCER_PRIMARY_PORT: String(pgbouncerPrimaryPort),
        PGBOUNCER_REPLICA_PORT: String(pgbouncerReplicaPort),
      },
      // Longer startup timeout for HA stack (replicas need to sync)
      startupTimeout: 180000,
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
  runPostgresDataTests(() => ({
    url: gatewayUrl,
    client,
    testName,
    execInService: env.execInService,
    postgresServiceName: "postgres-primary",
  }));

  // PostgreSQL HA specific tests
  describe("PostgreSQL Replication", () => {
    it("primary has active replication connections", async () => {
      // Query pg_stat_replication to verify replicas are connected
      const result = await env.execInService("postgres-primary", [
        "psql",
        "-U",
        "gateway",
        "-d",
        "gateway",
        "-t",
        "-c",
        "SELECT count(*) FROM pg_stat_replication",
      ]);

      expect(result.exitCode).toBe(0);
      // Should have at least 1 replica connected (ideally 2)
      const count = parseInt(result.output.trim(), 10);
      expect(count).toBeGreaterThanOrEqual(1);
    });

    it("replica-1 is in recovery mode", async () => {
      // Verify replica is actually a standby
      const result = await env.execInService("postgres-replica-1", [
        "psql",
        "-U",
        "gateway",
        "-d",
        "gateway",
        "-t",
        "-c",
        "SELECT pg_is_in_recovery()",
      ]);

      expect(result.exitCode).toBe(0);
      expect(result.output.trim()).toBe("t"); // true = in recovery (standby)
    });

    it("replica-2 is in recovery mode", async () => {
      // Verify second replica is also a standby
      const result = await env.execInService("postgres-replica-2", [
        "psql",
        "-U",
        "gateway",
        "-d",
        "gateway",
        "-t",
        "-c",
        "SELECT pg_is_in_recovery()",
      ]);

      expect(result.exitCode).toBe(0);
      expect(result.output.trim()).toBe("t"); // true = in recovery (standby)
    });

    it("data written to primary is replicated to replica", async () => {
      // Write a test value to primary
      const testValue = `ha-test-${Date.now()}`;
      const writeResult = await env.execInService("postgres-primary", [
        "psql",
        "-U",
        "gateway",
        "-d",
        "gateway",
        "-c",
        `CREATE TABLE IF NOT EXISTS ha_test (value TEXT); INSERT INTO ha_test (value) VALUES ('${testValue}')`,
      ]);
      expect(writeResult.exitCode).toBe(0);

      // Wait for replication (streaming replication should be near-instant)
      await new Promise((resolve) => setTimeout(resolve, 1000));

      // Read from replica to verify replication
      const readResult = await env.execInService("postgres-replica-1", [
        "psql",
        "-U",
        "gateway",
        "-d",
        "gateway",
        "-t",
        "-c",
        `SELECT value FROM ha_test WHERE value = '${testValue}'`,
      ]);

      expect(readResult.exitCode).toBe(0);
      expect(readResult.output).toContain(testValue);

      // Cleanup
      await env.execInService("postgres-primary", [
        "psql",
        "-U",
        "gateway",
        "-d",
        "gateway",
        "-c",
        "DROP TABLE IF EXISTS ha_test",
      ]);
    });
  });

  describe("PgBouncer Connection Pooling", () => {
    it("pgbouncer-primary is accepting connections", async () => {
      // PgBouncer should be healthy and accepting connections
      const result = await env.execInService("pgbouncer-primary", [
        "pg_isready",
        "-h",
        "localhost",
        "-p",
        "6432",
      ]);

      expect(result.exitCode).toBe(0);
    });

    it("pgbouncer-replica is accepting connections", async () => {
      // PgBouncer for replicas should also be healthy
      const result = await env.execInService("pgbouncer-replica", [
        "pg_isready",
        "-h",
        "localhost",
        "-p",
        "6432",
      ]);

      expect(result.exitCode).toBe(0);
    });
  });

  describe("Cache Subsystem (Redis)", () => {
    it("health endpoint reports cache subsystem healthy", async () => {
      const response = await healthCheck({ client });

      expect(response.response.status).toBe(200);
      expect(response.data?.status).toBe("healthy");
      expect(response.data?.subsystems?.cache).toBeDefined();
      expect(response.data?.subsystems?.cache?.healthy).toBe(true);
    });
  });

  describe("Database Subsystem", () => {
    it("health endpoint reports database subsystem healthy", async () => {
      const response = await healthCheck({ client });

      expect(response.response.status).toBe(200);
      expect(response.data?.status).toBe("healthy");
      expect(response.data?.subsystems?.database).toBeDefined();
      expect(response.data?.subsystems?.database?.healthy).toBe(true);
    });
  });
});
