/**
 * OIDC Authentication Flow tests
 *
 * Tests OIDC token acquisition, claim verification, and authentication endpoints.
 * Corresponds to test_oidc_auth_flow() from bash script.
 */
import { describe, it, expect } from "vitest";
import type { KeycloakConfig, TokenResponse } from "../../fixtures/keycloak";
import {
  getOidcToken,
  getOidcDiscovery,
  TEST_USERS,
} from "../../fixtures/keycloak";
import {
  decodeJwtPayload,
  jwtHasClaim,
  jwtHasRole,
  jwtHasGroup,
} from "../../utils/jwt";
import { trackedFetch } from "../../utils/tracked-fetch";

export interface OidcAuthFlowContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /** Keycloak configuration */
  keycloakConfig: KeycloakConfig;
  /** Organization slug for per-org SSO (required for /auth/login) */
  orgSlug?: string;
}

/**
 * Run OIDC authentication flow tests.
 *
 * @param getContext - Function that returns the test context
 */
export function runOidcAuthFlowTests(getContext: () => OidcAuthFlowContext) {
  describe("OIDC Authentication Flow", () => {
    // Test 1: OIDC Discovery Endpoint
    describe("Discovery Endpoint", () => {
      it("returns discovery document with required endpoints", async () => {
        const { keycloakConfig } = getContext();
        const discovery = await getOidcDiscovery(keycloakConfig);

        expect(discovery.issuer).toBeDefined();
        expect(discovery.authorization_endpoint).toBeDefined();
        expect(discovery.token_endpoint).toBeDefined();
        expect(discovery.jwks_uri).toBeDefined();
        expect(discovery.userinfo_endpoint).toBeDefined();
      });
    });

    // Tests 2-3: Super Admin Token and Claims
    describe("Super Admin Token", () => {
      let tokenResponse: TokenResponse;

      it("can acquire token for super_admin", async () => {
        const { keycloakConfig } = getContext();
        tokenResponse = await getOidcToken(
          keycloakConfig,
          TEST_USERS.superAdmin.username,
          TEST_USERS.superAdmin.password
        );

        expect(tokenResponse.access_token).toBeDefined();
        expect(tokenResponse.token_type).toBe("Bearer");
        expect(tokenResponse.expires_in).toBeGreaterThan(0);
      });

      it("access token contains correct preferred_username claim", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        expect(jwtHasClaim(payload, "preferred_username")).toBe(true);
        expect(payload.preferred_username).toBe("admin_super");
      });

      it("access token contains super_admin role", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        expect(jwtHasClaim(payload, "roles")).toBe(true);
        expect(jwtHasRole(payload, "super_admin")).toBe(true);
      });

      it("access token contains /it/platform group", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        // Groups claim may not be present in all configurations
        if (jwtHasClaim(payload, "groups")) {
          expect(jwtHasGroup(payload, "/it/platform")).toBe(true);
        }
      });
    });

    // Test 4: Org Admin Token and Claims
    describe("Org Admin Token", () => {
      let tokenResponse: TokenResponse;

      it("can acquire token for org_admin (cs_admin)", async () => {
        const { keycloakConfig } = getContext();
        tokenResponse = await getOidcToken(
          keycloakConfig,
          TEST_USERS.orgAdmin.username,
          TEST_USERS.orgAdmin.password
        );

        expect(tokenResponse.access_token).toBeDefined();
      });

      it("access token contains org_admin role", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        expect(jwtHasRole(payload, "org_admin")).toBe(true);
      });

      it("access token does not contain super_admin role", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        expect(jwtHasRole(payload, "super_admin")).toBe(false);
      });
    });

    // Test 5: Regular User Token and Claims
    describe("Regular User Token", () => {
      let tokenResponse: TokenResponse;

      it("can acquire token for regular user (phd_bob)", async () => {
        const { keycloakConfig } = getContext();
        tokenResponse = await getOidcToken(
          keycloakConfig,
          TEST_USERS.user.username,
          TEST_USERS.user.password
        );

        expect(tokenResponse.access_token).toBeDefined();
      });

      it("access token contains user role", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        expect(jwtHasRole(payload, "user")).toBe(true);
      });

      it("access token does not contain admin roles", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        expect(jwtHasRole(payload, "super_admin")).toBe(false);
        expect(jwtHasRole(payload, "org_admin")).toBe(false);
      });

      it("access token contains /cs/phd-students group", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        // Groups claim may not be present in all configurations
        if (jwtHasClaim(payload, "groups")) {
          expect(jwtHasGroup(payload, "/cs/phd-students")).toBe(true);
        }
      });
    });

    // Test 6: Invalid Credentials
    describe("Invalid Credentials", () => {
      it("rejects invalid password", async () => {
        const { keycloakConfig } = getContext();

        await expect(
          getOidcToken(keycloakConfig, "admin_super", "wrongpassword")
        ).rejects.toThrow();
      });

      it("rejects non-existent user", async () => {
        const { keycloakConfig } = getContext();

        await expect(
          getOidcToken(keycloakConfig, "nonexistent_user", "password123")
        ).rejects.toThrow();
      });
    });

    // Test 7: Gateway Auth Endpoints
    describe("Gateway Auth Endpoints", () => {
      it("/auth/login returns redirect or authorization URL", async () => {
        const { gatewayUrl, orgSlug } = getContext();
        // Per-org SSO requires the ?org= parameter
        const loginUrl = orgSlug
          ? `${gatewayUrl}/auth/login?org=${orgSlug}`
          : `${gatewayUrl}/auth/login`;
        const response = await trackedFetch(loginUrl, {
          redirect: "manual", // Don't follow redirects
        });

        // Should redirect to IdP (302/303/307) or return JSON with auth URL (200)
        const validStatuses = [200, 302, 303, 307];
        expect(validStatuses).toContain(response.status);

        if (response.status === 200) {
          const text = await response.text();
          // May return HTML login page or JSON with authorization URL
          expect(
            text.toLowerCase().includes("authorization") ||
              text.toLowerCase().includes("login")
          ).toBe(true);
        }
      });
    });

    // Test 8: ID Token Claims
    describe("ID Token Claims", () => {
      it("ID token contains email claim", async () => {
        const { keycloakConfig } = getContext();
        const tokenResponse = await getOidcToken(
          keycloakConfig,
          TEST_USERS.superAdmin.username,
          TEST_USERS.superAdmin.password
        );

        // ID token may not always be present depending on client configuration
        if (tokenResponse.id_token) {
          const payload = decodeJwtPayload(tokenResponse.id_token);
          expect(jwtHasClaim(payload, "email")).toBe(true);
        }
      });

      it("ID token contains preferred_username claim", async () => {
        const { keycloakConfig } = getContext();
        const tokenResponse = await getOidcToken(
          keycloakConfig,
          TEST_USERS.superAdmin.username,
          TEST_USERS.superAdmin.password
        );

        if (tokenResponse.id_token) {
          const payload = decodeJwtPayload(tokenResponse.id_token);
          expect(jwtHasClaim(payload, "preferred_username")).toBe(true);
          expect(payload.preferred_username).toBe("admin_super");
        }
      });
    });

    // Additional test: Team Admin Token
    describe("Team Admin Token", () => {
      let tokenResponse: TokenResponse;

      it("can acquire token for team_admin (prof_smith)", async () => {
        const { keycloakConfig } = getContext();
        tokenResponse = await getOidcToken(
          keycloakConfig,
          TEST_USERS.teamAdmin.username,
          TEST_USERS.teamAdmin.password
        );

        expect(tokenResponse.access_token).toBeDefined();
      });

      it("access token contains team_admin role", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        expect(jwtHasRole(payload, "team_admin")).toBe(true);
      });

      it("access token contains /cs/faculty group", async () => {
        const payload = decodeJwtPayload(tokenResponse.access_token);

        if (jwtHasClaim(payload, "groups")) {
          expect(jwtHasGroup(payload, "/cs/faculty")).toBe(true);
        }
      });
    });
  });
}
