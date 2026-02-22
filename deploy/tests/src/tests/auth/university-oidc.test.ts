/**
 * University Deployment Tests (OIDC + CEL RBAC)
 *
 * Tests the gateway with Keycloak OIDC authentication and CEL-based RBAC policies.
 * This is the most comprehensive auth test, covering:
 *   - OIDC authentication flow (token acquisition, claim verification)
 *   - University data setup (org, teams, users, projects, API keys, SSO mappings)
 *   - CEL policy enforcement (role-based access control)
 *   - API policy enforcement (model access, token limits)
 *   - Usage tracking and streaming
 *   - RAG/Vector store tests
 *   - API key scoping
 *   - Org RBAC policy management
 *
 * Migrated from deploy/test-e2e.sh test_university() function.
 *
 * Test credentials (from Keycloak realm):
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
  waitForKeycloak,
  setupKeycloakTestContext,
  type KeycloakTestContext,
  type KeycloakConfig,
  getOidcDiscovery,
  getOidcToken,
  TEST_USERS,
} from "../../fixtures";
import {
  setupUniversityData,
  type UniversityDataContext,
  TEAMS,
} from "../../fixtures/university-data";
import {
  setupUniversityOidcDeploymentData,
  configureOidcSso,
  configureSsoGroupMappings,
  restartGatewayForOidc,
  type UniversityOidcDeploymentContext,
} from "../../fixtures/university-oidc-deployment-data";
import { createConfig } from "../../client/client";
import type { Client } from "../../client/client";
import { runHealthCheckTests } from "../shared/health-checks";
import { runOidcAuthFlowTests } from "../shared/oidc-auth-flow";
import {
  runCelPolicyEnforcementTests,
  createOidcCelContext,
} from "../shared/cel-policy-enforcement";
import {
  runApiPolicyEnforcementTests,
  createOidcApiPolicyContext,
} from "../shared/api-policy-enforcement";
import { runUsageAndStreamingTests } from "../shared/usage-and-streaming";
import { runRagEndpointTests } from "../shared/rag-endpoints";
import { runApiKeyScopingTests } from "../shared/api-key-scoping";
import {
  runOrgRbacPolicyCrudTests,
  runOrgRbacPolicyEnforcementTests,
  createOidcOrgRbacContext,
} from "../shared/org-rbac-policies";
import { runScimProvisioningTests } from "../shared/scim-provisioning";
import { runSessionManagementTests } from "../shared/session-management";
import { runMultiAuthTests } from "../shared/multi-auth";
import { runAnalyticsTests } from "../shared/analytics";
import { trackedFetch } from "../../utils/tracked-fetch";

describe("University Deployment (OIDC + CEL RBAC)", () => {
  let env: StartedComposeEnvironment;
  let gatewayUrl: string;
  let keycloakUrl: string;
  let client: Client;
  let keycloakConfig: KeycloakConfig;
  let keycloakContext: KeycloakTestContext;
  let universityData: UniversityDataContext;
  let oidcDeployment: UniversityOidcDeploymentContext;
  let adminToken: string;

  // Helper to create authenticated client with Bearer token
  const authenticatedClient = (token: string) =>
    createTrackedClient(
      createConfig({
        baseUrl: gatewayUrl,
        headers: { Authorization: `Bearer ${token}` },
      }),
    );

  // Helper to create authenticated client with API key
  const _apiKeyClient = (apiKey: string) =>
    createTrackedClient(
      createConfig({
        baseUrl: gatewayUrl,
        headers: { Authorization: `Bearer ${apiKey}` },
      }),
    );

  beforeAll(async () => {
    // Port allocation for parallel test execution - each test file uses unique host ports
    // to avoid conflicts. Testcontainers still uses getMappedPort() for the actual URL.
    const gatewayPort = 8089;
    const keycloakPort = 8181;
    const vaultPort = 8201; // Must be set to avoid default 8200 conflicts
    const qdrantHttpPort = 6340;
    const qdrantGrpcPort = 6341;

    env = await startComposeEnvironment({
      projectName: "hadrian-e2e-university",
      composeFile: "docker-compose.university.yml",
      // Use healthcheck wait strategy for keycloak since it takes longer to start
      serviceWaitStrategies: {
        keycloak: { type: "healthcheck" },
      },
      waitForServices: {
        gateway: { port: 8080, path: "/health" },
      },
      env: {
        GATEWAY_PORT: String(gatewayPort),
        KEYCLOAK_PORT: String(keycloakPort),
        VAULT_PORT: String(vaultPort),
        QDRANT_HTTP_PORT: String(qdrantHttpPort),
        QDRANT_GRPC_PORT: String(qdrantGrpcPort),
      },
      startupTimeout: 180_000, // 3 minutes for Keycloak to start
    });

    gatewayUrl = env.getServiceUrl("gateway", 8080);
    keycloakUrl = env.getServiceUrl("keycloak", 8080);
    keycloakConfig = { baseUrl: keycloakUrl };

    // Wait for Keycloak to be fully ready (realm import can take time)
    await waitForKeycloak(keycloakUrl, { maxRetries: 90, retryInterval: 2000 });

    // Use createTrackedClient for API coverage tracking
    client = createTrackedClient(createConfig({ baseUrl: gatewayUrl }));

    // Step 1: Set up OIDC deployment data using bootstrap API key
    // This creates org, teams, and configures OIDC SSO (but doesn't restart yet)
    oidcDeployment = await setupUniversityOidcDeploymentData(gatewayUrl);
    await configureOidcSso(
      gatewayUrl,
      keycloakUrl,
      "http://keycloak:8080" // Internal URL for gateway to reach Keycloak
    );
    await configureSsoGroupMappings(gatewayUrl, oidcDeployment.teamIds);

    // Step 2: Create first admin user with bootstrap key
    // Bootstrap auth is disabled after first user is created
    const firstUserResponse = await fetch(`${gatewayUrl}/admin/v1/users`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${oidcDeployment.bootstrapKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        external_id: TEST_USERS.superAdmin.username,
        email: "admin.super@university.edu",
        name: "Super Admin",
      }),
    });
    if (!firstUserResponse.ok) {
      throw new Error(`Failed to create first admin user: ${firstUserResponse.status}`);
    }
    const firstUserData = await firstUserResponse.json();

    // Add first user to organization
    await fetch(`${gatewayUrl}/admin/v1/organizations/university/members`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${oidcDeployment.bootstrapKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ user_id: firstUserData.id }),
    });

    // Step 3: Get token for the first admin user (bootstrap is now disabled)
    const firstUserToken = await getOidcToken(
      keycloakConfig,
      TEST_USERS.superAdmin.username,
      TEST_USERS.superAdmin.password
    );

    // Step 4: Set up remaining university data using the admin user's token
    // Skip the first user since they're already created
    const adminClient = authenticatedClient(firstUserToken.access_token);
    universityData = await setupUniversityData(adminClient, {
      orgId: oidcDeployment.orgId,
      teamIds: oidcDeployment.teamIds,
      skipUsers: [TEST_USERS.superAdmin.username], // Skip first user
      firstUserId: firstUserData.id, // Pass the first user's ID
    });

    // Step 5: Restart gateway to load SSO configuration
    await restartGatewayForOidc(env, gatewayUrl);

    // Step 6: Set up Keycloak test context with pre-fetched tokens for all roles
    keycloakContext = await setupKeycloakTestContext(keycloakUrl);

    // Step 7: Use super admin token for admin API access
    adminToken = keycloakContext.tokens.superAdmin.access_token;
  });

  afterAll(async () => {
    await env?.stop();
  });

  // =========================================================================
  // Health Endpoints (auth-enabled mode still allows health endpoints)
  // =========================================================================
  runHealthCheckTests(() => ({ url: gatewayUrl, client }));

  // =========================================================================
  // OIDC Authentication Flow Tests
  // =========================================================================
  runOidcAuthFlowTests(() => ({
    gatewayUrl,
    keycloakConfig,
    orgSlug: "university", // Per-org SSO requires org parameter
  }));

  // =========================================================================
  // Keycloak OIDC Integration
  // =========================================================================
  describe("Keycloak OIDC Integration", () => {
    it("OIDC discovery endpoint returns valid configuration", async () => {
      const discovery = await getOidcDiscovery(keycloakConfig);

      expect(discovery.issuer).toBeDefined();
      expect(discovery.issuer).toContain("/realms/hadrian");
      expect(discovery.authorization_endpoint).toBeDefined();
      expect(discovery.token_endpoint).toBeDefined();
      expect(discovery.jwks_uri).toBeDefined();
    });
  });

  // =========================================================================
  // University Data Verification
  // =========================================================================
  describe("University Data Setup Verification", () => {
    it("organization was created", async () => {
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations`,
        {
          headers: { Authorization: `Bearer ${adminToken}` },
        },
      );

      expect(response.status).toBe(200);
      const data = await response.json();
      const university = data.data?.find(
        (org: { slug: string }) => org.slug === "university",
      );
      expect(university).toBeDefined();
      expect(university.name).toBe("State University");
    });

    it("teams were created in university org", async () => {
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/teams`,
        {
          headers: { Authorization: `Bearer ${adminToken}` },
        },
      );

      expect(response.status).toBe(200);
      const data = await response.json();
      const teamSlugs = data.data?.map((t: { slug: string }) => t.slug) || [];

      // Verify all expected teams exist
      for (const team of TEAMS) {
        expect(teamSlugs).toContain(team.slug);
      }
    });

    it("users were created", async () => {
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/users?limit=50`,
        {
          headers: { Authorization: `Bearer ${adminToken}` },
        },
      );

      expect(response.status).toBe(200);
      const data = await response.json();
      const externalIds =
        data.data?.map((u: { external_id: string }) => u.external_id) || [];

      // Verify key users exist
      expect(externalIds).toContain("admin_super");
      expect(externalIds).toContain("phd_bob");
      expect(externalIds).toContain("prof_smith");
      expect(externalIds).toContain("cs_admin");
    });

    it("API keys were created", () => {
      // Verify the API keys from setup
      expect(universityData.apiKeys.org.key).toBeDefined();
      expect(universityData.apiKeys.budget.key).toBeDefined();
      expect(universityData.apiKeys.user.key).toBeDefined();
    });
  });

  // =========================================================================
  // CEL Policy Enforcement Tests
  // =========================================================================
  runCelPolicyEnforcementTests(() =>
    createOidcCelContext(gatewayUrl, keycloakContext),
  );

  // =========================================================================
  // API Policy Enforcement Tests
  // =========================================================================
  runApiPolicyEnforcementTests(() =>
    createOidcApiPolicyContext(gatewayUrl, keycloakContext),
  );

  // =========================================================================
  // Usage Tracking and Streaming Tests
  // =========================================================================
  runUsageAndStreamingTests(() => ({
    gatewayUrl,
    adminToken,
    apiKeys: universityData.apiKeys,
  }));

  // =========================================================================
  // RAG/Vector Store Tests
  // =========================================================================
  runRagEndpointTests(() => ({
    gatewayUrl,
    orgId: universityData.orgId,
    apiKey: universityData.apiKeys.org.key,
  }));

  // =========================================================================
  // API Key Scoping Tests
  // =========================================================================
  runApiKeyScopingTests(() => ({
    gatewayUrl,
    adminToken,
    orgId: universityData.orgId,
  }));

  // =========================================================================
  // Org RBAC Policy Tests
  // =========================================================================
  runOrgRbacPolicyCrudTests(() =>
    createOidcOrgRbacContext(gatewayUrl, keycloakContext),
  );

  runOrgRbacPolicyEnforcementTests(() =>
    createOidcOrgRbacContext(gatewayUrl, keycloakContext),
  );

  // =========================================================================
  // SCIM Provisioning Tests
  // =========================================================================
  runScimProvisioningTests(() => ({
    gatewayUrl,
    adminClient: authenticatedClient(adminToken),
    orgSlug: "university", // Use org slug, not UUID
  }));

  // =========================================================================
  // Session Management Tests
  // =========================================================================
  runSessionManagementTests(() => ({
    gatewayUrl,
    adminToken,
    userId: universityData.userIds["phd_bob"], // Use a known user from the test data
  }));

  // =========================================================================
  // Multi-Auth Tests (API key + JWT format-based detection)
  // =========================================================================
  runMultiAuthTests(() => ({
    gatewayUrl,
    apiKey: universityData.apiKeys.org.key,
    jwtToken: keycloakContext.tokens.user.access_token,
  }));

  // =========================================================================
  // Provider Analytics Tests
  // =========================================================================
  runAnalyticsTests(() => ({
    gatewayUrl,
    adminToken: keycloakContext.tokens.superAdmin.access_token,
    apiKey: universityData.apiKeys.org.key,
    providerName: "test",
    modelName: "test/test-model",
  }));
});
