/**
 * PostgreSQL data persistence tests
 *
 * Tests that verify data is correctly persisted in PostgreSQL.
 * Used by deployments with PostgreSQL: postgres, postgres-ha.
 */
import { describe, it, expect } from "vitest";
import type { Client } from "../../client/client";
import type { ExecResult } from "../../fixtures/compose";
import { orgMemberList } from "../../client";

export interface PostgresDataContext {
  url: string;
  client: Client;
  /** Test name prefix used for organization slug (e.g., "postgres" -> "postgres-org") */
  testName: string;
  /** Execute commands in a container (required for direct PostgreSQL verification) */
  execInService: (serviceName: string, command: string[]) => Promise<ExecResult>;
  /** PostgreSQL service name in docker-compose (default: "postgres") */
  postgresServiceName?: string;
}

/**
 * Run PostgreSQL data persistence tests.
 * @param getContext - Function that returns the test context. Called lazily to ensure
 *                     the context is available after beforeAll setup completes.
 */
export function runPostgresDataTests(getContext: () => PostgresDataContext) {
  describe("PostgreSQL Data Persistence", () => {
    it("persists organization data in PostgreSQL", async () => {
      const { testName, execInService, postgresServiceName = "postgres" } =
        getContext();
      // The admin-api-crud tests create an organization with slug "{testName}-org"
      // Verify it exists in PostgreSQL directly
      const orgSlug = `${testName}-org`;

      const result = await execInService(postgresServiceName, [
        "psql",
        "-U",
        "gateway",
        "-d",
        "gateway",
        "-t",
        "-c",
        `SELECT slug FROM organizations WHERE slug='${orgSlug}'`,
      ]);

      expect(result.exitCode).toBe(0);
      expect(result.output).toContain(orgSlug);
    });

    it("persists project data in PostgreSQL", async () => {
      const { testName, execInService, postgresServiceName = "postgres" } =
        getContext();
      // Verify the project created by admin-api-crud tests exists
      const result = await execInService(postgresServiceName, [
        "psql",
        "-U",
        "gateway",
        "-d",
        "gateway",
        "-t",
        "-c",
        `SELECT p.slug FROM projects p
         JOIN organizations o ON p.org_id = o.id
         WHERE o.slug='${testName}-org' AND p.slug='test-project'`,
      ]);

      expect(result.exitCode).toBe(0);
      expect(result.output).toContain("test-project");
    });

    it("persists user data in PostgreSQL", async () => {
      const { testName, execInService, postgresServiceName = "postgres" } =
        getContext();
      // Verify the user created by admin-api-crud tests exists
      const result = await execInService(postgresServiceName, [
        "psql",
        "-U",
        "gateway",
        "-d",
        "gateway",
        "-t",
        "-c",
        `SELECT external_id FROM users WHERE external_id='${testName}-user'`,
      ]);

      expect(result.exitCode).toBe(0);
      expect(result.output).toContain(`${testName}-user`);
    });
  });

  describe("Organization Members", () => {
    it("returns members list for organization", async () => {
      const { client, testName } = getContext();
      const orgSlug = `${testName}-org`;

      const response = await orgMemberList({
        client,
        path: { org_slug: orgSlug },
      });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      // Should return a paginated response with data array
      expect(response.data?.data).toBeDefined();
      expect(Array.isArray(response.data?.data)).toBe(true);
    });
  });
}
