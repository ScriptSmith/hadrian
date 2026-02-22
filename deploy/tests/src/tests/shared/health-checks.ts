/**
 * Health endpoint tests (Tests 1-4, 10 from bash)
 *
 * Tests the basic health, liveness, readiness endpoints and API documentation.
 */
import { describe, it, expect } from "vitest";
import type { Client } from "../../client/client";
import { healthCheck, healthLiveness, healthReadiness } from "../../client";
import { trackedFetch } from "../../utils/tracked-fetch";

export interface HealthCheckContext {
  url: string;
  client: Client;
}

export function runHealthCheckTests(getContext: () => HealthCheckContext) {
  describe("Health Endpoints", () => {
    // Test 1: Health endpoint
    it("returns healthy status with database subsystem", async () => {
      const { client } = getContext();
      const response = await healthCheck({ client });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      expect(response.data?.status).toBe("healthy");
      // Database info is in subsystems.database
      expect(response.data?.subsystems.database).toBeDefined();
    });

    // Test 2: Liveness endpoint
    it("returns 200 on liveness endpoint", async () => {
      const { client } = getContext();
      const response = await healthLiveness({ client });

      expect(response.response.status).toBe(200);
    });

    // Test 3: Readiness endpoint
    it("returns 200 on readiness endpoint", async () => {
      const { client } = getContext();
      const response = await healthReadiness({ client });

      expect(response.response.status).toBe(200);
    });

    // Test 4: OpenAPI spec
    // Note: /openapi.json is a meta-endpoint that returns the API spec itself,
    // so it's not included in the spec. We test it with trackedFetch for coverage.
    it("serves OpenAPI spec at /openapi.json", async () => {
      const { url } = getContext();
      const response = await trackedFetch(`${url}/openapi.json`);
      const data = await response.json();

      expect(response.status).toBe(200);
      expect(data).toHaveProperty("openapi");
    });

    // Test 10: Docs endpoint
    // Note: Docs endpoints are HTML pages, not API endpoints, so we test them with trackedFetch.
    it("serves API documentation", async () => {
      const { url } = getContext();
      // Try common documentation paths
      const paths = ["/api/docs", "/docs", "/api-docs", "/swagger"];
      let found = false;

      for (const path of paths) {
        const response = await trackedFetch(`${url}${path}`, {
          redirect: "follow",
        });
        if (response.ok) {
          const text = await response.text();
          // The docs page should contain either "scalar" or "openapi" references
          expect(text.toLowerCase()).toMatch(/scalar|openapi|swagger/i);
          found = true;
          break;
        }
      }

      expect(found).toBe(true);
    });
  });
}
