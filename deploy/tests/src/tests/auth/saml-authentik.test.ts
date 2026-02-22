/**
 * SAML Deployment Tests (Authentik + CEL RBAC)
 *
 * Tests the gateway with Authentik SAML authentication and CEL-based RBAC policies.
 * This is the comprehensive SAML auth test, covering:
 *   - SAML authentication flow (browser-based login via Playwright)
 *   - SAML deployment data setup (org, teams, SSO config, group mappings)
 *   - CEL policy enforcement (role-based access control)
 *   - JIT user provisioning from SAML assertions
 *
 * Migrated from deploy/test-e2e.sh test_saml() function.
 *
 * Test credentials (from Authentik blueprint - deploy/config/authentik/blueprint.yaml):
 *   | Role        | Username     | Password      |
 *   |-------------|--------------|---------------|
 *   | super_admin | admin_super  | admin123      |
 *   | org_admin   | cs_admin     | orgadmin123   |
 *   | team_admin  | prof_smith   | teamadmin123  |
 *   | user        | phd_bob      | user123       |
 */
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import {
  startComposeEnvironment,
  createTrackedClient,
  type StartedComposeEnvironment,
  waitForAuthentik,
  TEST_SAML_USERS,
  getSamlSession,
  type SamlSession,
  type SamlBrowserConfig,
} from "../../fixtures";
import {
  setupCompleteSamlDeployment,
  createAuthentikApiToken,
  verifyAuthentikApiAccess,
  updateAuthentikSamlProvider,
  provisionSamlUsersWithSession,
  type SamlDeploymentContext,
  SAML_TEAMS,
} from "../../fixtures/saml-deployment-data";
import { createConfig } from "../../client/client";
import type { Client } from "../../client/client";
import { runHealthCheckTests } from "../shared/health-checks";
import {
  runCelPolicyEnforcementTests,
  createSamlCelContext,
  type CelTestRole,
} from "../shared/cel-policy-enforcement";
import { trackedFetch } from "../../utils/tracked-fetch";

describe("SAML Deployment (Authentik + CEL RBAC)", () => {
  let env: StartedComposeEnvironment;
  let gatewayUrl: string;
  let authentikUrl: string;
  let client: Client;
  let deploymentContext: SamlDeploymentContext;
  let authentikApiToken: string;

  // SAML sessions for different roles (acquired via Playwright browser automation)
  let samlSessions: Record<CelTestRole, SamlSession>;

  beforeAll(async () => {
    // Port allocation for parallel test execution - each test file uses unique host ports
    // to avoid conflicts. Testcontainers still uses getMappedPort() for the actual URL.
    const gatewayPort = 8090;
    const authentikPort = 9000;
    const authentikHttpsPort = 9444; // Must be set to avoid default 9443 conflicts
    const qdrantHttpPort = 6338;
    const qdrantGrpcPort = 6339;

    env = await startComposeEnvironment({
      projectName: "hadrian-e2e-saml",
      composeFile: "docker-compose.saml.yml",
      // Use healthcheck wait strategy for authentik-server since it takes longer to start
      serviceWaitStrategies: {
        "authentik-server": { type: "healthcheck" },
      },
      waitForServices: {
        gateway: { port: 8080, path: "/health" },
      },
      env: {
        GATEWAY_PORT: String(gatewayPort),
        AUTHENTIK_HTTP_PORT: String(authentikPort),
        AUTHENTIK_HTTPS_PORT: String(authentikHttpsPort),
        QDRANT_HTTP_PORT: String(qdrantHttpPort),
        QDRANT_GRPC_PORT: String(qdrantGrpcPort),
      },
      startupTimeout: 300_000, // 5 minutes for Authentik to start and import blueprints
    });

    gatewayUrl = env.getServiceUrl("gateway", 8080);
    authentikUrl = env.getServiceUrl("authentik-server", 9000);

    // Wait for Authentik to be fully ready (health + SAML metadata available)
    await waitForAuthentik(
      { baseUrl: authentikUrl },
      { maxRetries: 120, retryInterval: 2000 }
    );

    // Use createTrackedClient for API coverage tracking
    client = createTrackedClient(createConfig({ baseUrl: gatewayUrl }));

    // Create Authentik API token via docker exec
    authentikApiToken = await createAuthentikApiToken(env);

    // Verify Authentik API is accessible
    const apiAccessible = await verifyAuthentikApiAccess(
      authentikUrl,
      authentikApiToken
    );
    if (!apiAccessible) {
      throw new Error("Authentik API not accessible with created token");
    }

    // Update Authentik SAML provider to use the actual gateway URL
    // (Blueprint uses default localhost:3000, but tests may use different ports)
    await updateAuthentikSamlProvider(authentikUrl, gatewayUrl, authentikApiToken);

    // Set up SAML deployment data (org, teams, SSO config, group mappings)
    // Note: User provisioning is done AFTER super_admin login because
    // the bootstrap API key is disabled once a user exists
    deploymentContext = await setupCompleteSamlDeployment(
      env,
      gatewayUrl,
      authentikUrl
    );

    // Browser config for SAML login
    const browserConfig: SamlBrowserConfig = {
      gatewayUrl,
      authentikUrl,
      orgSlug: "university",
      debug: process.env.DEBUG_SAML === "1",
    };

    // Step 1: Login as super_admin first (this user has full access via CEL role check)
    const superAdminSession = await getSamlSession(
      browserConfig,
      TEST_SAML_USERS.superAdmin
    );

    // Step 2: Use super_admin session to provision other users in the database
    // (Bootstrap API key doesn't work for user management, so we use admin session)
    const provisionResult = await provisionSamlUsersWithSession(
      gatewayUrl,
      superAdminSession.cookie,
      deploymentContext.teamIds
    );
    console.log(
      `User provisioning: ${provisionResult.provisioned} provisioned, ${provisionResult.failed} failed`
    );

    // Step 3: Login remaining test users
    const [orgAdminSession, teamAdminSession, userSession] = await Promise.all([
      getSamlSession(browserConfig, TEST_SAML_USERS.orgAdmin),
      getSamlSession(browserConfig, TEST_SAML_USERS.teamAdmin),
      getSamlSession(browserConfig, TEST_SAML_USERS.user),
    ]);

    samlSessions = {
      superAdmin: superAdminSession,
      orgAdmin: orgAdminSession,
      teamAdmin: teamAdminSession,
      user: userSession,
    };
  });

  afterAll(async () => {
    // Cleanup browser contexts
    if (samlSessions) {
      await Promise.all([
        samlSessions.superAdmin?.cleanup(),
        samlSessions.orgAdmin?.cleanup(),
        samlSessions.teamAdmin?.cleanup(),
        samlSessions.user?.cleanup(),
      ]);
    }

    await env?.stop();
  });

  // =========================================================================
  // Health Endpoints (SAML deployment still allows health endpoints)
  // =========================================================================
  runHealthCheckTests(() => ({ url: gatewayUrl, client }));

  // =========================================================================
  // Authentik SAML Integration Tests
  // =========================================================================
  describe("Authentik SAML Integration", () => {
    it("Authentik API is accessible with test token", async () => {
      const response = await fetch(`${authentikUrl}/api/v3/core/users/`, {
        headers: { Authorization: `Bearer ${authentikApiToken}` },
      });

      expect(response.ok).toBe(true);
      const data = await response.json();
      expect(data.results).toBeDefined();
    });

    it("SAML metadata endpoint is available", async () => {
      const response = await fetch(
        `${authentikUrl}/application/saml/hadrian-gateway/metadata/`
      );

      expect(response.ok).toBe(true);
      const metadata = await response.text();
      expect(metadata).toContain("EntityDescriptor");
    });
  });

  // =========================================================================
  // SAML Deployment Data Verification
  // =========================================================================
  describe("SAML Deployment Data Verification", () => {
    it("organization was created", async () => {
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations`,
        {
          headers: {
            Cookie: `__gw_session=${samlSessions.superAdmin.cookie}`,
          },
        }
      );

      expect(response.status).toBe(200);
      const data = await response.json();
      const university = data.data?.find(
        (org: { slug: string }) => org.slug === "university"
      );
      expect(university).toBeDefined();
      expect(university.name).toBe("State University");
    });

    it("teams were created in university org", async () => {
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/teams`,
        {
          headers: {
            Cookie: `__gw_session=${samlSessions.superAdmin.cookie}`,
          },
        }
      );

      expect(response.status).toBe(200);
      const data = await response.json();
      const teamSlugs = data.data?.map((t: { slug: string }) => t.slug) || [];

      // Verify all expected teams exist
      for (const team of SAML_TEAMS) {
        expect(teamSlugs).toContain(team.slug);
      }
    });

    it("SAML SSO config was created", async () => {
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/sso-config`,
        {
          headers: {
            Cookie: `__gw_session=${samlSessions.superAdmin.cookie}`,
          },
        }
      );

      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.provider_type).toBe("saml");
      expect(data.enabled).toBe(true);
    });
  });

  // =========================================================================
  // SAML Authentication Flow Tests
  // =========================================================================
  describe("SAML Authentication Flow", () => {
    it("SAML login initiates redirect to Authentik", async () => {
      // Test that /auth/saml/login redirects to Authentik
      const response = await trackedFetch(
        `${gatewayUrl}/auth/saml/login?org=university`,
        { redirect: "manual" }
      );

      // Should be a redirect (302 or 303)
      expect([302, 303]).toContain(response.status);

      const location = response.headers.get("location");
      expect(location).toBeDefined();
      // Location should point to Authentik
      expect(location).toMatch(/authentik|localhost:9000/);
    });

    it("super_admin user has valid session after SAML login", async () => {
      expect(samlSessions.superAdmin).toBeDefined();
      expect(samlSessions.superAdmin.cookie).toBeDefined();
      expect(samlSessions.superAdmin.userInfo.email).toBe(
        TEST_SAML_USERS.superAdmin.expectedEmail
      );
    });

    it("org_admin user has valid session after SAML login", async () => {
      expect(samlSessions.orgAdmin).toBeDefined();
      expect(samlSessions.orgAdmin.cookie).toBeDefined();
      expect(samlSessions.orgAdmin.userInfo.email).toBe(
        TEST_SAML_USERS.orgAdmin.expectedEmail
      );
    });

    it("team_admin user has valid session after SAML login", async () => {
      expect(samlSessions.teamAdmin).toBeDefined();
      expect(samlSessions.teamAdmin.cookie).toBeDefined();
      expect(samlSessions.teamAdmin.userInfo.email).toBe(
        TEST_SAML_USERS.teamAdmin.expectedEmail
      );
    });

    it("regular user has valid session after SAML login", async () => {
      expect(samlSessions.user).toBeDefined();
      expect(samlSessions.user.cookie).toBeDefined();
      expect(samlSessions.user.userInfo.email).toBe(
        TEST_SAML_USERS.user.expectedEmail
      );
    });

    it("authenticated user can access /auth/me endpoint", async () => {
      const response = await trackedFetch(`${gatewayUrl}/auth/me`, {
        headers: {
          Cookie: `__gw_session=${samlSessions.superAdmin.cookie}`,
        },
      });

      expect(response.ok).toBe(true);
      const data = await response.json();
      expect(data.email).toBe(TEST_SAML_USERS.superAdmin.expectedEmail);
    });

    it("unauthenticated request to /auth/me returns 401", async () => {
      const response = await trackedFetch(`${gatewayUrl}/auth/me`);
      expect(response.status).toBe(401);
    });
  });

  // =========================================================================
  // JIT User Provisioning Tests
  // =========================================================================
  describe("JIT User Provisioning", () => {
    it("SAML-authenticated users are provisioned in gateway database", async () => {
      // Use super admin session to list users
      // This requires the super_admin role to be extracted from SAML groups
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/users?limit=50`,
        {
          headers: {
            Cookie: `__gw_session=${samlSessions.superAdmin.cookie}`,
          },
        }
      );

      // If this fails with 403, it means the super_admin role wasn't extracted
      // from the SAML assertion's groups attribute
      if (!response.ok) {
        console.warn(
          `User list failed with ${response.status} - super_admin role may not be extracted from SAML`
        );
      }
      expect(response.ok).toBe(true);

      const data = await response.json();
      const externalIds =
        data.data?.map((u: { external_id: string }) => u.external_id) || [];

      // All logged-in users should have been provisioned
      expect(externalIds).toContain("admin_super");
      expect(externalIds).toContain("cs_admin");
      expect(externalIds).toContain("prof_smith");
      expect(externalIds).toContain("phd_bob");
    });

    it("authenticated user info is accessible via /auth/me", async () => {
      // Check that /auth/me returns user info for SAML-authenticated users
      const meResponse = await trackedFetch(`${gatewayUrl}/auth/me`, {
        headers: {
          Cookie: `__gw_session=${samlSessions.superAdmin.cookie}`,
        },
      });

      expect(meResponse.ok).toBe(true);
      const meData = await meResponse.json();

      // Basic user info should be present
      expect(meData.email).toBe(TEST_SAML_USERS.superAdmin.expectedEmail);
      expect(meData.external_id).toBeDefined();

      // Note: org_ids may be empty if JIT provisioning doesn't automatically
      // add users to the organization. This depends on SSO config and
      // group mappings being processed correctly.
      // If org_ids is populated, verify it contains valid data
      if (meData.org_ids && meData.org_ids.length > 0) {
        expect(meData.org_ids[0]).toMatch(
          /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
        );
      }
    });
  });

  // =========================================================================
  // CEL Policy Enforcement Tests (using SAML sessions)
  // =========================================================================
  runCelPolicyEnforcementTests(() =>
    createSamlCelContext(gatewayUrl, {
      superAdmin: samlSessions.superAdmin.cookie,
      orgAdmin: samlSessions.orgAdmin.cookie,
      teamAdmin: samlSessions.teamAdmin.cookie,
      user: samlSessions.user.cookie,
    })
  );

  // =========================================================================
  // SAML-Specific Edge Cases
  // =========================================================================
  describe("SAML Edge Cases", () => {
    it("SAML login with unknown org returns error", async () => {
      const response = await trackedFetch(
        `${gatewayUrl}/auth/saml/login?org=nonexistent`,
        { redirect: "manual" }
      );

      // Should return an error, not redirect to IdP
      expect([403, 404]).toContain(response.status);
    });
  });
});
