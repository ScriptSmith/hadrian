/**
 * Admin API CRUD tests (Tests 5-9, 12-22 from bash)
 *
 * Tests organization, project, user, team, and API key management.
 */
import { describe, it, expect } from "vitest";
import type { Client } from "../../client/client";
import {
  organizationList,
  organizationCreate,
  organizationGet,
  projectCreate,
  userCreate,
  teamCreate,
  teamGet,
  teamList,
  teamUpdate,
  teamDelete,
  teamMemberAdd,
  teamMemberList,
  teamMemberUpdate,
  teamMemberRemove,
  apiKeyCreate,
} from "../../client";

export interface AdminApiCrudContext {
  url: string;
  client: Client;
  testName: string;
}

/**
 * Run admin API CRUD tests.
 * @param getContext - Function that returns the test context. Called lazily to ensure
 *                     the context is available after beforeAll setup completes.
 */
export function runAdminApiCrudTests(getContext: () => AdminApiCrudContext) {
  describe("Admin API CRUD", () => {
    let orgId: string;
    let userId: string;
    let teamId: string;

    // Test 5: List organizations
    it("can list organizations", async () => {
      const { client } = getContext();
      const response = await organizationList({ client });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      // Response is paginated with data array
      expect(response.data?.data).toBeDefined();
      expect(Array.isArray(response.data?.data)).toBe(true);
    });

    // Test 6: Create organization
    it("can create an organization", async () => {
      const { client, testName } = getContext();
      const response = await organizationCreate({
        client,
        body: {
          slug: `${testName}-org`,
          name: `Test Organization for ${testName}`,
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data).toBeDefined();
      expect(response.data?.slug).toBe(`${testName}-org`);

      // Store org ID for later tests
      orgId = response.data!.id;
    });

    // Test 7: Get organization
    it("can get an organization by slug", async () => {
      if (!orgId) {
        throw new Error(
          "Test prerequisite failed: orgId not set. The 'can create an organization' test must pass first."
        );
      }
      const { client, testName } = getContext();
      const response = await organizationGet({
        client,
        path: { slug: `${testName}-org` },
      });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      expect(response.data?.name).toBe(`Test Organization for ${testName}`);
    });

    // Test 8: Create project
    it("can create a project in an organization", async () => {
      const { client, testName } = getContext();
      const response = await projectCreate({
        client,
        path: { org_slug: `${testName}-org` },
        body: {
          slug: "test-project",
          name: "Test Project",
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data).toBeDefined();
      expect(response.data?.slug).toBe("test-project");
    });

    // Test 9: Create API key (org-scoped)
    it("can create an org-scoped API key", async () => {
      if (!orgId) {
        throw new Error(
          "Test prerequisite failed: orgId not set. The 'can create an organization' test must pass first."
        );
      }
      const { client } = getContext();
      const response = await apiKeyCreate({
        client,
        body: {
          name: "Test Key",
          owner: {
            type: "organization",
            org_id: orgId,
          },
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data).toBeDefined();
      expect(response.data?.key).toMatch(/^gw_/);
    });

    // Test 12: Create user
    it("can create a user", async () => {
      const { client, testName } = getContext();
      const response = await userCreate({
        client,
        body: {
          external_id: `${testName}-user`,
          email: `${testName}@example.com`,
          name: "Test User",
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data).toBeDefined();
      expect(response.data?.external_id).toBe(`${testName}-user`);

      // Store user ID for later tests
      userId = response.data!.id;
    });

    // Test 13: Create team
    it("can create a team", async () => {
      const { client, testName } = getContext();
      const response = await teamCreate({
        client,
        path: { org_slug: `${testName}-org` },
        body: {
          slug: "test-team",
          name: "Test Team",
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data).toBeDefined();
      expect(response.data?.slug).toBe("test-team");

      // Store team ID for later tests
      teamId = response.data!.id;
    });

    // Test 14: Get team
    it("can get a team by slug", async () => {
      const { client, testName } = getContext();
      const response = await teamGet({
        client,
        path: {
          org_slug: `${testName}-org`,
          team_slug: "test-team",
        },
      });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      expect(response.data?.name).toBe("Test Team");
    });

    // Test 15: List teams
    it("can list teams in an organization", async () => {
      const { client, testName } = getContext();
      const response = await teamList({
        client,
        path: { org_slug: `${testName}-org` },
      });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      expect(response.data?.data).toBeDefined();
      expect(Array.isArray(response.data?.data)).toBe(true);

      const teamSlugs = response.data?.data.map((t) => t.slug);
      expect(teamSlugs).toContain("test-team");
    });

    // Test 16: Update team
    it("can update a team", async () => {
      const { client, testName } = getContext();
      const response = await teamUpdate({
        client,
        path: {
          org_slug: `${testName}-org`,
          team_slug: "test-team",
        },
        body: {
          name: "Updated Team Name",
        },
      });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      expect(response.data?.name).toBe("Updated Team Name");
    });

    // Test 17: Add team member
    it("can add a member to a team", async () => {
      if (!userId) {
        throw new Error(
          "Test prerequisite failed: userId not set. The 'can create a user' test must pass first."
        );
      }
      const { client, testName } = getContext();
      const response = await teamMemberAdd({
        client,
        path: {
          org_slug: `${testName}-org`,
          team_slug: "test-team",
        },
        body: {
          user_id: userId,
          role: "member",
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data).toBeDefined();
      expect(response.data?.role).toBe("member");
    });

    // Test 18: List team members
    it("can list team members", async () => {
      const { client, testName } = getContext();
      const response = await teamMemberList({
        client,
        path: {
          org_slug: `${testName}-org`,
          team_slug: "test-team",
        },
      });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      expect(response.data?.data).toBeDefined();
      expect(Array.isArray(response.data?.data)).toBe(true);

      const memberIds = response.data?.data.map((m) => m.user_id);
      expect(memberIds).toContain(userId);
    });

    // Test 19: Update team member role
    it("can update a team member's role", async () => {
      const { client, testName } = getContext();
      const response = await teamMemberUpdate({
        client,
        path: {
          org_slug: `${testName}-org`,
          team_slug: "test-team",
          user_id: userId,
        },
        body: {
          role: "admin",
        },
      });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      expect(response.data?.role).toBe("admin");
    });

    // Test 20: Remove team member
    it("can remove a team member", async () => {
      const { client, testName } = getContext();
      const response = await teamMemberRemove({
        client,
        path: {
          org_slug: `${testName}-org`,
          team_slug: "test-team",
          user_id: userId,
        },
      });

      expect(response.response.status).toBe(200);

      // Verify member was removed
      const listResponse = await teamMemberList({
        client,
        path: {
          org_slug: `${testName}-org`,
          team_slug: "test-team",
        },
      });

      const memberIds = listResponse.data?.data.map((m) => m.user_id);
      expect(memberIds).not.toContain(userId);
    });

    // Test 21: Create team-scoped API key
    it("can create a team-scoped API key", async () => {
      if (!teamId) {
        throw new Error(
          "Test prerequisite failed: teamId not set. The 'can create a team' test must pass first."
        );
      }
      const { client } = getContext();
      const response = await apiKeyCreate({
        client,
        body: {
          name: "Team API Key",
          owner: {
            type: "team",
            team_id: teamId,
          },
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data).toBeDefined();
      expect(response.data?.key).toMatch(/^gw_/);
    });

    // Test 22: Delete team
    it("can delete a team", async () => {
      const { client, testName } = getContext();
      const response = await teamDelete({
        client,
        path: {
          org_slug: `${testName}-org`,
          team_slug: "test-team",
        },
      });

      expect(response.response.status).toBe(200);

      // Verify team was deleted (should return 404)
      const getResponse = await teamGet({
        client,
        path: {
          org_slug: `${testName}-org`,
          team_slug: "test-team",
        },
      });

      expect(getResponse.response.status).toBe(404);
    });
  });
}
