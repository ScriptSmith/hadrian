/**
 * Org RBAC Policy Tests
 *
 * Tests organization-level RBAC policy management and enforcement.
 * Migrated from test_org_rbac_policies() and test_org_rbac_policy_enforcement()
 * in deploy/test-e2e.sh.
 *
 * This module is auth-agnostic and works with both:
 * - OIDC (Bearer token authentication via Keycloak)
 * - SAML (Cookie-based session authentication via Authentik)
 *
 * CRUD Tests (from test_org_rbac_policies):
 *   1. List policies (initially empty or existing)
 *   2. Validate CEL expression (valid)
 *   3. Validate CEL expression (invalid - syntax error)
 *   4. Create policy
 *   5. Get policy by ID
 *   6. List policies (verify created)
 *   7. Update policy
 *   8. List versions (verify version history)
 *   9. Simulate policy evaluation
 *   10. Rollback to previous version
 *   11. Delete policy + verify deletion (404)
 *
 * Enforcement Tests (from test_org_rbac_policy_enforcement):
 *   1. Baseline - phd_bob CAN use test-model (default allow)
 *   2. Create org DENY policy for test-model
 *   3. Verify tightening - phd_bob is DENIED (403)
 *   4. Verify exclusion - cs_admin CAN still use test-model (200)
 *   5. Delete the deny policy
 *   6. Verify restored - phd_bob CAN use test-model again
 *
 * Test credentials:
 *   | Role        | Username     | Password      |
 *   |-------------|--------------|---------------|
 *   | super_admin | admin_super  | admin123      |
 *   | org_admin   | cs_admin     | orgadmin123   |
 *   | user        | phd_bob      | user123       |
 */
import { describe, it, expect } from "vitest";
import type { KeycloakTestContext } from "../../fixtures/keycloak";
import { trackedFetch } from "../../utils/tracked-fetch";

/**
 * Role types for org RBAC policy tests.
 */
export type OrgRbacTestRole = "superAdmin" | "orgAdmin" | "user";

/**
 * Auth-agnostic context for org RBAC policy tests.
 * Works with both OIDC tokens and SAML cookies.
 */
export interface OrgRbacPolicyTestContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /**
   * Get authentication headers for a specific role.
   * Returns either `Authorization: Bearer <token>` for OIDC
   * or `Cookie: __gw_session=<cookie>` for SAML.
   */
  getAuthHeaders: (role: OrgRbacTestRole) => Record<string, string>;
}

/**
 * Create an org RBAC policy context from an OIDC/Keycloak test context.
 * Maps Keycloak tokens to auth headers.
 */
export function createOidcOrgRbacContext(
  gatewayUrl: string,
  keycloakContext: KeycloakTestContext,
): OrgRbacPolicyTestContext {
  return {
    gatewayUrl,
    getAuthHeaders: (role: OrgRbacTestRole) => {
      const tokenMap = {
        superAdmin: keycloakContext.tokens.superAdmin.access_token,
        orgAdmin: keycloakContext.tokens.orgAdmin.access_token,
        user: keycloakContext.tokens.user.access_token,
      };
      return { Authorization: `Bearer ${tokenMap[role]}` };
    },
  };
}

/**
 * Create an org RBAC policy context from SAML session cookies.
 * Maps role names to session cookies.
 */
export function createSamlOrgRbacContext(
  gatewayUrl: string,
  sessionCookies: Record<OrgRbacTestRole, string>,
): OrgRbacPolicyTestContext {
  return {
    gatewayUrl,
    getAuthHeaders: (role: OrgRbacTestRole) => {
      return { Cookie: `__gw_session=${sessionCookies[role]}` };
    },
  };
}

/**
 * Run org RBAC policy CRUD tests.
 * Tests match test_org_rbac_policies() from bash script exactly.
 *
 * @param getContext - Function that returns the test context
 */
export function runOrgRbacPolicyCrudTests(
  getContext: () => OrgRbacPolicyTestContext,
) {
  describe("Org RBAC Policy CRUD", () => {
    let createdPolicyId: string | undefined;

    // =========================================================================
    // Test 1: List policies (should be empty initially or have existing)
    // =========================================================================
    it("can list policies", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies`,
        {
          headers: getAuthHeaders("superAdmin"),
        },
      );
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data).toHaveProperty("data");
      expect(Array.isArray(data.data)).toBe(true);
    });

    // =========================================================================
    // Test 2: Validate CEL expression (valid)
    // =========================================================================
    it("validates valid CEL expression", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/rbac-policies/validate`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("superAdmin"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            condition: '"org_admin" in subject.roles',
          }),
        },
      );
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.valid).toBe(true);
    });

    // =========================================================================
    // Test 3: Validate CEL expression (invalid - syntax error)
    // =========================================================================
    it("rejects invalid CEL expression", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/rbac-policies/validate`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("superAdmin"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            condition: "subject.roles.exists(r, r ==", // Missing closing paren
          }),
        },
      );
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.valid).toBe(false);
    });

    // =========================================================================
    // Test 4: Create policy
    // =========================================================================
    it("creates a policy", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("superAdmin"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            name: "test-org-policy",
            description: "Test policy for E2E tests",
            resource: "*",
            action: "*",
            condition: '"test_role" in subject.roles',
            effect: "allow",
            priority: 50,
            enabled: true,
          }),
        },
      );
      expect(response.status).toBe(201);
      const data = await response.json();
      expect(data.id).toBeDefined();
      expect(data.name).toBe("test-org-policy");
      createdPolicyId = data.id;
    });

    // =========================================================================
    // Test 5: Get policy by ID
    // =========================================================================
    it("retrieves policy by ID", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      expect(createdPolicyId).toBeDefined();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies/${createdPolicyId}`,
        {
          headers: getAuthHeaders("superAdmin"),
        },
      );
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.name).toBe("test-org-policy");
      expect(data.description).toBe("Test policy for E2E tests");
    });

    // =========================================================================
    // Test 6: List policies (verify created)
    // =========================================================================
    it("lists policies and includes created policy", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies`,
        {
          headers: getAuthHeaders("superAdmin"),
        },
      );
      expect(response.status).toBe(200);
      const data = await response.json();
      const policy = data.data.find(
        (p: { name: string }) => p.name === "test-org-policy",
      );
      expect(policy).toBeDefined();
    });

    // =========================================================================
    // Test 7: Update policy
    // =========================================================================
    it("updates a policy", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      expect(createdPolicyId).toBeDefined();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies/${createdPolicyId}`,
        {
          method: "PATCH",
          headers: {
            ...getAuthHeaders("superAdmin"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            description: "Updated test policy",
            condition: '"updated_role" in subject.roles',
            priority: 60,
          }),
        },
      );
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data.description).toBe("Updated test policy");
      expect(data.priority).toBe(60);
    });

    // =========================================================================
    // Test 8: List versions (verify version history)
    // =========================================================================
    it("lists policy versions", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      expect(createdPolicyId).toBeDefined();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies/${createdPolicyId}/versions`,
        {
          headers: getAuthHeaders("superAdmin"),
        },
      );
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data).toHaveProperty("data");
      expect(Array.isArray(data.data)).toBe(true);
      // Should have at least 2 versions (create + update)
      expect(data.data.length).toBeGreaterThanOrEqual(2);
    });

    // =========================================================================
    // Test 9: Simulate policy evaluation
    // =========================================================================
    it("simulates policy evaluation", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies/simulate`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("superAdmin"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            subject: {
              roles: ["updated_role"],
            },
            context: {
              resource_type: "teams",
              action: "read",
            },
          }),
        },
      );
      expect(response.status).toBe(200);
      const data = await response.json();
      // Response includes rbac_enabled, allowed, org_policies_evaluated, system_policies_evaluated
      expect(data).toHaveProperty("rbac_enabled");
      expect(data).toHaveProperty("allowed");
      expect(data).toHaveProperty("org_policies_evaluated");
    });

    // =========================================================================
    // Test 10: Rollback to previous version
    // =========================================================================
    it("rolls back policy to previous version", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      expect(createdPolicyId).toBeDefined();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies/${createdPolicyId}/rollback`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("superAdmin"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            target_version: 1,
          }),
        },
      );
      expect(response.status).toBe(200);
      const data = await response.json();
      expect(data).toHaveProperty("version");
      // After rollback, priority should be restored to original value
      expect(data.priority).toBe(50);
    });

    // =========================================================================
    // Test 11: Delete policy
    // =========================================================================
    it("deletes a policy", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      expect(createdPolicyId).toBeDefined();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies/${createdPolicyId}`,
        {
          method: "DELETE",
          headers: getAuthHeaders("superAdmin"),
        },
      );
      expect(response.status).toBe(200);
    });

    // =========================================================================
    // Test 12: Verify deletion (404)
    // =========================================================================
    it("returns 404 for deleted policy", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      expect(createdPolicyId).toBeDefined();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies/${createdPolicyId}`,
        {
          headers: getAuthHeaders("superAdmin"),
        },
      );
      expect(response.status).toBe(404);
    });
  });
}

/**
 * Run org RBAC policy enforcement tests.
 * Tests match test_org_rbac_policy_enforcement() from bash script exactly.
 *
 * Scenario:
 * 1. Baseline: phd_bob CAN use "test-model" (API default_effect = allow)
 * 2. Create org DENY policy for test-model (restricts non-org_admin users)
 * 3. Verify tightening: phd_bob is DENIED using test-model
 * 4. Verify exclusion: cs_admin (org_admin role) CAN still use test-model
 * 5. Delete the deny policy
 * 6. Verify restored: phd_bob CAN use test-model again
 *
 * @param getContext - Function that returns the test context
 */
export function runOrgRbacPolicyEnforcementTests(
  getContext: () => OrgRbacPolicyTestContext,
) {
  describe("Org RBAC Policy Enforcement", () => {
    let denyPolicyId: string | undefined;

    // =========================================================================
    // Test 1: Baseline - phd_bob CAN use test-model (default allow)
    // =========================================================================
    it("baseline: regular user can use test-model", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      const response = await trackedFetch(
        `${gatewayUrl}/api/v1/chat/completions`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("user"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            model: "test/test-model",
            messages: [{ role: "user", content: "Hello" }],
          }),
        },
      );
      expect(response.status).toBe(200);
    });

    // =========================================================================
    // Test 2: Create org DENY policy for test-model
    // =========================================================================
    it("creates deny policy for test-model", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("superAdmin"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            name: "restrict-test-model",
            description: "Only org admins can use test-model",
            resource: "model",
            action: "use",
            condition:
              "context.model == 'test/test-model' && !('org_admin' in subject.roles)",
            effect: "deny",
            priority: 100,
            enabled: true,
          }),
        },
      );
      expect(response.status).toBe(201);
      const data = await response.json();
      expect(data.id).toBeDefined();
      denyPolicyId = data.id;
    });

    // =========================================================================
    // Test 3: Verify tightening - phd_bob is DENIED (403)
    // =========================================================================
    it("regular user is denied after policy creation", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      expect(denyPolicyId).toBeDefined();

      const response = await trackedFetch(
        `${gatewayUrl}/api/v1/chat/completions`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("user"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            model: "test/test-model",
            messages: [{ role: "user", content: "Hello" }],
          }),
        },
      );
      expect(response.status).toBe(403);
    });

    // =========================================================================
    // Test 4: Verify exclusion - cs_admin (org_admin) CAN still use test-model
    // =========================================================================
    it("org admin can still use test-model (excluded from deny)", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      const response = await trackedFetch(
        `${gatewayUrl}/api/v1/chat/completions`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("orgAdmin"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            model: "test/test-model",
            messages: [{ role: "user", content: "Hello" }],
          }),
        },
      );
      expect(response.status).toBe(200);
    });

    // =========================================================================
    // Test 5: Delete the deny policy
    // =========================================================================
    it("deletes the deny policy", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      expect(denyPolicyId).toBeDefined();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/organizations/university/rbac-policies/${denyPolicyId}`,
        {
          method: "DELETE",
          headers: getAuthHeaders("superAdmin"),
        },
      );
      expect(response.status).toBe(200);
    });

    // =========================================================================
    // Test 6: Verify restored - phd_bob CAN use test-model again
    // =========================================================================
    it("regular user can use test-model again after policy deletion", async () => {
      const { gatewayUrl, getAuthHeaders } = getContext();
      const response = await trackedFetch(
        `${gatewayUrl}/api/v1/chat/completions`,
        {
          method: "POST",
          headers: {
            ...getAuthHeaders("user"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            model: "test/test-model",
            messages: [{ role: "user", content: "Hello" }],
          }),
        },
      );
      expect(response.status).toBe(200);
    });
  });
}
