/**
 * CEL Policy Enforcement Tests
 *
 * Tests CEL-based RBAC policy enforcement across different user roles.
 * Migrated from test_cel_policy_enforcement() in deploy/test-e2e.sh.
 *
 * This module is auth-agnostic and works with both:
 * - OIDC (Bearer token authentication via Keycloak)
 * - SAML (Cookie-based session authentication via Authentik)
 *
 * Test scenarios (matching bash script exactly):
 *   1. Super Admin - Full access to all resources
 *   2. Org Admin - Organization management within own org
 *   3. Team Admin - Own team access only, cannot modify other teams
 *   4. Regular User - Limited access, cannot create/delete resources
 *   5. Deny Policies - Self-delete prevention for all users
 *
 * Test credentials:
 *   | Role        | Username     | Password      |
 *   |-------------|--------------|---------------|
 *   | super_admin | admin_super  | admin123      |
 *   | org_admin   | cs_admin     | orgadmin123   |
 *   | team_admin  | prof_smith   | teamadmin123  |
 *   | user        | phd_bob      | user123       |
 */
import { describe, it, expect } from "vitest";
import type { KeycloakTestContext } from "../../fixtures/keycloak";
import { trackedFetch } from "../../utils/tracked-fetch";

/**
 * Role types for CEL policy tests.
 */
export type CelTestRole = "superAdmin" | "orgAdmin" | "teamAdmin" | "user";

/**
 * Auth-agnostic context for CEL policy enforcement tests.
 * Works with both OIDC tokens and SAML cookies.
 */
export interface CelPolicyEnforcementContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /**
   * Get authentication headers for a specific role.
   * Returns either `Authorization: Bearer <token>` for OIDC
   * or `Cookie: __gw_session=<cookie>` for SAML.
   */
  getAuthHeaders: (role: CelTestRole) => Record<string, string>;
}

/**
 * Create a CEL policy context from an OIDC/Keycloak test context.
 * Maps Keycloak tokens to auth headers.
 */
export function createOidcCelContext(
  gatewayUrl: string,
  keycloakContext: KeycloakTestContext
): CelPolicyEnforcementContext {
  return {
    gatewayUrl,
    getAuthHeaders: (role: CelTestRole) => {
      const tokenMap = {
        superAdmin: keycloakContext.tokens.superAdmin.access_token,
        orgAdmin: keycloakContext.tokens.orgAdmin.access_token,
        teamAdmin: keycloakContext.tokens.teamAdmin.access_token,
        user: keycloakContext.tokens.user.access_token,
      };
      return { Authorization: `Bearer ${tokenMap[role]}` };
    },
  };
}

/**
 * Create a CEL policy context from SAML session cookies.
 * Maps role names to session cookies.
 */
export function createSamlCelContext(
  gatewayUrl: string,
  sessionCookies: Record<CelTestRole, string>
): CelPolicyEnforcementContext {
  return {
    gatewayUrl,
    getAuthHeaders: (role: CelTestRole) => {
      return { Cookie: `__gw_session=${sessionCookies[role]}` };
    },
  };
}

/**
 * Legacy interface for backwards compatibility.
 * @deprecated Use CelPolicyEnforcementContext with getAuthHeaders instead.
 */
export interface LegacyCelPolicyEnforcementContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /** Pre-fetched tokens for different roles */
  keycloakContext: KeycloakTestContext;
}

/**
 * Run CEL policy enforcement tests.
 * Tests match test_cel_policy_enforcement() from bash script exactly.
 *
 * @param getContext - Function that returns the test context
 */
export function runCelPolicyEnforcementTests(
  getContext: () => CelPolicyEnforcementContext
) {
  describe("CEL Policy Enforcement", () => {
    // =========================================================================
    // Test 1: Super Admin - Full Access
    // =========================================================================
    describe("Super Admin Full Access", () => {
      it("can list all organizations", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations`,
          {
            headers: getAuthHeaders("superAdmin"),
          }
        );
        expect(response.status).toBe(200);
      });

      it("can read university organization", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university`,
          {
            headers: getAuthHeaders("superAdmin"),
          }
        );
        expect(response.status).toBe(200);
      });

      it("can list all teams", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams`,
          {
            headers: getAuthHeaders("superAdmin"),
          }
        );
        expect(response.status).toBe(200);
      });
    });

    // =========================================================================
    // Test 2: Org Admin - Organization Management
    // =========================================================================
    describe("Org Admin Access", () => {
      it("can list organizations", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations`,
          {
            headers: getAuthHeaders("orgAdmin"),
          }
        );
        expect(response.status).toBe(200);
      });

      it("can read own organization", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university`,
          {
            headers: getAuthHeaders("orgAdmin"),
          }
        );
        expect(response.status).toBe(200);
      });

      it("can list teams in own org", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams`,
          {
            headers: getAuthHeaders("orgAdmin"),
          }
        );
        expect(response.status).toBe(200);
      });

      it("can create a team in own org", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("orgAdmin"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              slug: "cel-test-team",
              name: "CEL Test Team",
            }),
          }
        );

        // Accept 200 or 201 for successful creation
        expect([200, 201]).toContain(response.status);

        // Clean up - delete the test team
        await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams/cel-test-team`,
          {
            method: "DELETE",
            headers: getAuthHeaders("orgAdmin"),
          }
        );
      });
    });

    // =========================================================================
    // Test 3: Team Admin - Own Team Access Only
    // =========================================================================
    describe("Team Admin Boundary Enforcement", () => {
      it("can list teams", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams`,
          {
            headers: getAuthHeaders("teamAdmin"),
          }
        );
        expect(response.status).toBe(200);
      });

      it("can read own team (cs-faculty)", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams/cs-faculty`,
          {
            headers: getAuthHeaders("teamAdmin"),
          }
        );
        expect(response.status).toBe(200);
      });

      it("cannot update another team (cs-phd-students)", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams/cs-phd-students`,
          {
            method: "PATCH",
            headers: {
              ...getAuthHeaders("teamAdmin"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({ name: "Hacked PhD Students" }),
          }
        );
        expect(response.status).toBe(403);
      });

      it("cannot create new teams", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("teamAdmin"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({ slug: "new-team", name: "New Team" }),
          }
        );
        expect(response.status).toBe(403);
      });
    });

    // =========================================================================
    // Test 4: Regular User - Limited Access
    // =========================================================================
    describe("Regular User Boundary Enforcement", () => {
      it("can list organizations they belong to", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations`,
          {
            headers: getAuthHeaders("user"),
          }
        );
        expect(response.status).toBe(200);
      });

      it("cannot create organizations", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("user"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({ slug: "hacked-org", name: "Hacked Org" }),
          }
        );
        expect(response.status).toBe(403);
      });

      it("cannot create teams", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("user"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({ slug: "hacked-team", name: "Hacked Team" }),
          }
        );
        expect(response.status).toBe(403);
      });

      it("cannot delete teams", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations/university/teams/cs-faculty`,
          {
            method: "DELETE",
            headers: getAuthHeaders("user"),
          }
        );
        expect(response.status).toBe(403);
      });
    });

    // =========================================================================
    // Additional Tests (beyond bash script)
    // =========================================================================
    describe("Additional Boundary Tests", () => {
      it("org admin cannot create new organizations", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/organizations`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("orgAdmin"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({ slug: "new-org", name: "New Organization" }),
          }
        );
        // Only super_admin can create organizations
        expect(response.status).toBe(403);
      });

      // Note: org_admin CAN delete their own organization (verified - returns 200).
      // This test is intentionally skipped because:
      // 1. It's destructive and would break subsequent tests
      // 2. May be intended behavior (org admins manage their org lifecycle)
      // If this is a security concern, it should be addressed in RBAC policy config.

      it("only super_admin can list users", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        // Super admin can list users
        const superAdminResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/users?limit=50`,
          {
            headers: getAuthHeaders("superAdmin"),
          }
        );
        expect(superAdminResponse.status).toBe(200);

        // Org admin cannot list users
        const orgAdminResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/users?limit=50`,
          {
            headers: getAuthHeaders("orgAdmin"),
          }
        );
        expect(orgAdminResponse.status).toBe(403);

        // Regular user cannot list users
        const userResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/users?limit=50`,
          {
            headers: getAuthHeaders("user"),
          }
        );
        expect(userResponse.status).toBe(403);
      });
    });

    // =========================================================================
    // Test 5: Deny Policies - Self-Delete Prevention
    // =========================================================================
    describe("Deny Policies (Self-Delete Prevention)", () => {
      it("regular user cannot delete themselves", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        // Get user ID for phd_bob (using super_admin to list users)
        const usersResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/users?limit=50`,
          {
            headers: getAuthHeaders("superAdmin"),
          }
        );
        expect(usersResponse.ok).toBe(true);

        const usersData = await usersResponse.json();
        const phdBob = usersData.data?.find(
          (u: { external_id: string }) => u.external_id === "phd_bob"
        );

        expect(phdBob).toBeDefined();
        expect(phdBob.id).toBeDefined();

        // phd_bob tries to delete themselves
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/users/${phdBob.id}`,
          {
            method: "DELETE",
            headers: getAuthHeaders("user"),
          }
        );
        expect(response.status).toBe(403);
      });

      it("super admin cannot delete themselves", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        // Get user ID for admin_super
        const usersResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/users?limit=50`,
          {
            headers: getAuthHeaders("superAdmin"),
          }
        );
        expect(usersResponse.ok).toBe(true);

        const usersData = await usersResponse.json();
        const adminSuper = usersData.data?.find(
          (u: { external_id: string }) => u.external_id === "admin_super"
        );

        expect(adminSuper).toBeDefined();
        expect(adminSuper.id).toBeDefined();

        // admin_super tries to delete themselves
        const response = await trackedFetch(
          `${gatewayUrl}/admin/v1/users/${adminSuper.id}`,
          {
            method: "DELETE",
            headers: getAuthHeaders("superAdmin"),
          }
        );
        expect(response.status).toBe(403);
      });
    });
  });
}
