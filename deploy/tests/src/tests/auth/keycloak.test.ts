/**
 * Keycloak OIDC Deployment Tests
 *
 * Tests the gateway with Keycloak OIDC authentication enabled.
 * Migrated from deploy/test-e2e.sh test_keycloak() function.
 *
 * Note: Admin API tests are skipped because OIDC auth is enabled.
 * The university-oidc.test.ts provides comprehensive coverage for
 * OIDC + RBAC scenarios with proper token acquisition.
 */
import { describe, it, expect, beforeAll, afterAll } from "vitest";
import {
  startComposeEnvironment,
  createTrackedClient,
  type StartedComposeEnvironment,
  waitForKeycloak,
  getOidcDiscovery,
  type KeycloakConfig,
} from "../../fixtures";
import { createConfig } from "../../client/client";
import type { Client } from "../../client/client";
import { runHealthCheckTests } from "../shared/health-checks";
import { trackedFetch } from "../../utils/tracked-fetch";

/**
 * Bootstrap API key for initial setup (matches hadrian.keycloak.toml).
 */
const BOOTSTRAP_API_KEY = "gw_test_bootstrap_key_for_e2e";

/**
 * Set up the "default" organization with OIDC SSO configuration.
 * Uses bootstrap API key authentication.
 */
async function setupDefaultOrgWithOidcSso(
  gatewayUrl: string,
  _keycloakUrl: string
): Promise<void> {
  const authHeaders = { Authorization: `Bearer ${BOOTSTRAP_API_KEY}` };

  // 1. Create "default" organization
  const orgResponse = await fetch(`${gatewayUrl}/admin/v1/organizations`, {
    method: "POST",
    headers: { ...authHeaders, "Content-Type": "application/json" },
    body: JSON.stringify({ slug: "default", name: "Default Organization" }),
  });

  if (!orgResponse.ok && orgResponse.status !== 409) {
    throw new Error(`Failed to create default org: ${orgResponse.status}`);
  }

  // 2. Configure OIDC SSO
  // Keycloak advertises its issuer based on KC_HOSTNAME settings (localhost:8080),
  // regardless of what port we actually access it on via testcontainers.
  const keycloakAdvertisedIssuer = "http://localhost:8080/realms/hadrian";

  const ssoConfig = {
    provider_type: "oidc",
    enabled: true,
    issuer: keycloakAdvertisedIssuer,
    discovery_url: "http://keycloak:8080/realms/hadrian",
    client_id: "hadrian-gateway",
    client_secret: "test-secret-for-e2e",
    redirect_uri: `${gatewayUrl}/auth/callback`,
    identity_claim: "preferred_username",
    groups_claim: "groups",
    provisioning_enabled: true,
    create_users: true,
  };

  const ssoResponse = await fetch(
    `${gatewayUrl}/admin/v1/organizations/default/sso-config`,
    {
      method: "POST",
      headers: { ...authHeaders, "Content-Type": "application/json" },
      body: JSON.stringify(ssoConfig),
    }
  );

  if (!ssoResponse.ok && ssoResponse.status !== 409) {
    const error = await ssoResponse.text();
    throw new Error(`Failed to create OIDC SSO config: ${ssoResponse.status} ${error}`);
  }

  console.log("OIDC SSO configuration created for default organization");
}

describe("Keycloak OIDC Deployment", () => {
  let env: StartedComposeEnvironment;
  let gatewayUrl: string;
  let keycloakUrl: string;
  let client: Client;

  // Helper getters to ensure URLs are accessed after beforeAll completes
  const getGatewayUrl = () => gatewayUrl;
  const getKeycloakUrl = () => keycloakUrl;

  beforeAll(async () => {
    // Port allocation for parallel test execution - each test file uses unique host ports
    // to avoid conflicts. Testcontainers still uses getMappedPort() for the actual URL.
    const gatewayPort = 8088;
    const keycloakPort = 8180;

    env = await startComposeEnvironment({
      projectName: "hadrian-e2e-keycloak",
      composeFile: "docker-compose.keycloak.yml",
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
      },
      startupTimeout: 180_000, // 3 minutes for Keycloak to start
    });

    gatewayUrl = env.getServiceUrl("gateway", 8080);
    keycloakUrl = env.getServiceUrl("keycloak", 8080);

    // Wait for Keycloak to be fully ready (realm import can take time)
    await waitForKeycloak(keycloakUrl, { maxRetries: 60, retryInterval: 2000 });

    // Set up OIDC SSO using bootstrap API key
    // IMPORTANT: We must create the SSO config AFTER any gateway restarts, because
    // without a persistent secrets manager configured, the in-memory secret store
    // is cleared on restart. The SSO client secret would be lost if we created
    // the config before restarting.
    await setupDefaultOrgWithOidcSso(gatewayUrl, keycloakUrl);

    // Use createTrackedClient for API coverage tracking
    client = createTrackedClient(createConfig({ baseUrl: gatewayUrl }));
  });

  afterAll(async () => {
    await env?.stop();
  });

  // Run shared health check tests (auth-enabled mode still allows health endpoints)
  runHealthCheckTests(() => ({ url: gatewayUrl, client }));

  describe("Keycloak OIDC Integration", () => {
    it("OIDC discovery endpoint returns valid configuration", async () => {
      const config: KeycloakConfig = { baseUrl: getKeycloakUrl() };
      const discovery = await getOidcDiscovery(config);

      // Verify required OIDC discovery document fields
      expect(discovery.issuer).toBeDefined();
      expect(discovery.issuer).toContain("/realms/hadrian");
      expect(discovery.authorization_endpoint).toBeDefined();
      expect(discovery.token_endpoint).toBeDefined();
      expect(discovery.jwks_uri).toBeDefined();
      expect(discovery.userinfo_endpoint).toBeDefined();
    });

    it("OIDC discovery issuer matches expected realm", async () => {
      const config: KeycloakConfig = { baseUrl: getKeycloakUrl() };
      const discovery = await getOidcDiscovery(config);

      // The issuer should end with /realms/hadrian
      expect(discovery.issuer).toMatch(/\/realms\/hadrian$/);
    });

    it("JWKS endpoint is accessible", async () => {
      const kcUrl = getKeycloakUrl();
      const config: KeycloakConfig = { baseUrl: kcUrl };
      const discovery = await getOidcDiscovery(config);

      // The discovery document returns URLs with Keycloak's advertised hostname (localhost:8080)
      // which differs from the actual testcontainers mapped URL. Rewrite to use keycloakUrl.
      const jwksPath = new URL(discovery.jwks_uri).pathname;
      const jwksUrl = `${kcUrl}${jwksPath}`;

      const jwksResponse = await fetch(jwksUrl);
      expect(jwksResponse.ok).toBe(true);

      const jwks = await jwksResponse.json();
      expect(jwks).toHaveProperty("keys");
      expect(Array.isArray(jwks.keys)).toBe(true);
      expect(jwks.keys.length).toBeGreaterThan(0);
    });
  });

  describe("Gateway Auth-Enabled Mode", () => {
    // Note: Comprehensive unauthenticated access testing is done in university-oidc.test.ts
    // which provides full OIDC + RBAC coverage. Here we just verify basic auth endpoints work.

    it("auth login endpoint responds (redirect or JSON)", async () => {
      // /auth/login should either redirect to IdP (302) or return authorization URL
      // Per-org SSO requires the ?org= parameter to specify which org's SSO config to use
      const gwUrl = getGatewayUrl();
      const response = await trackedFetch(`${gwUrl}/auth/login?org=default`, {
        redirect: "manual",
      });

      // Accept either redirect (302/303/307) or success (200)
      expect([200, 302, 303, 307]).toContain(response.status);

      if (response.status === 200) {
        const data = await response.json();
        // If 200, should contain authorization URL
        expect(
          data.authorization_url || data.url || JSON.stringify(data)
        ).toMatch(/authorize|auth/i);
      }
    });
  });

  // Note: Comprehensive OIDC + RBAC testing is done in university-oidc.test.ts
  // which includes token acquisition, claim verification, and CEL policy enforcement.
});
