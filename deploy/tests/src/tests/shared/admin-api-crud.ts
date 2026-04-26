/**
 * Admin API CRUD tests (Tests 5-9, 12-22 from bash)
 *
 * Tests organization, project, user, team, and API key management.
 *
 * Each `it()` is order-independent: the org/user/team fixtures it depends on
 * are created in `beforeAll` rather than being a side-effect of an earlier
 * `it()`. Tests that exercise create/update/delete use scratch resources with
 * a unique suffix per test so they can run in any order without colliding.
 */
import { describe, beforeAll, it, expect } from "vitest";
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

let scratchCounter = 0;
const scratchSuffix = () => `${Date.now()}-${++scratchCounter}`;

/**
 * Run admin API CRUD tests.
 * @param getContext - Function that returns the test context. Called lazily to ensure
 *                     the context is available after beforeAll setup completes.
 */
export function runAdminApiCrudTests(getContext: () => AdminApiCrudContext) {
  describe("Admin API CRUD", () => {
    // Shared fixtures created once in beforeAll so individual tests don't
    // depend on the order of preceding `it()` blocks. The team is recreated
    // fresh for each test that mutates it (update/delete/member-management).
    let orgId: string;
    let userId: string;
    let orgSlug: string;

    beforeAll(async () => {
      const { client, testName } = getContext();
      orgSlug = `${testName}-org`;

      const orgRes = await organizationCreate({
        client,
        body: { slug: orgSlug, name: `Test Organization for ${testName}` },
      });
      if (!orgRes.data) {
        throw new Error(
          `beforeAll: organizationCreate returned no body (status ${orgRes.response.status})`
        );
      }
      orgId = orgRes.data.id;

      const userRes = await userCreate({
        client,
        body: {
          external_id: `${testName}-user`,
          email: `${testName}@example.com`,
          name: "Test User",
        },
      });
      if (!userRes.data) {
        throw new Error(
          `beforeAll: userCreate returned no body (status ${userRes.response.status})`
        );
      }
      userId = userRes.data.id;
    });

    /** Create a scratch team owned by the shared org for tests that mutate it. */
    const createScratchTeam = async (): Promise<{ slug: string; id: string }> => {
      const { client } = getContext();
      const slug = `scratch-team-${scratchSuffix()}`;
      const res = await teamCreate({
        client,
        path: { org_slug: orgSlug },
        body: { slug, name: `Scratch Team ${slug}` },
      });
      if (!res.data) {
        throw new Error(
          `createScratchTeam failed (status ${res.response.status})`
        );
      }
      return { slug, id: res.data.id };
    };

    // ── Org / project / user CRUD ─────────────────────────────────────────

    it("can list organizations", async () => {
      const { client } = getContext();
      const response = await organizationList({ client });

      expect(response.response.status).toBe(200);
      expect(response.data?.data).toBeDefined();
      expect(Array.isArray(response.data?.data)).toBe(true);
      // The shared beforeAll org should be visible in the list.
      const slugs = response.data?.data.map((o) => o.slug);
      expect(slugs).toContain(orgSlug);
    });

    it("can create an organization", async () => {
      const { client } = getContext();
      const slug = `scratch-org-${scratchSuffix()}`;
      const response = await organizationCreate({
        client,
        body: { slug, name: `Scratch Org ${slug}` },
      });

      expect(response.response.status).toBe(201);
      expect(response.data?.slug).toBe(slug);
      expect(response.data?.id).toBeDefined();
    });

    it("can get an organization by slug", async () => {
      const { client } = getContext();
      const response = await organizationGet({
        client,
        path: { slug: orgSlug },
      });

      expect(response.response.status).toBe(200);
      expect(response.data?.slug).toBe(orgSlug);
    });

    it("can create a project in an organization", async () => {
      const { client } = getContext();
      const slug = `scratch-project-${scratchSuffix()}`;
      const response = await projectCreate({
        client,
        path: { org_slug: orgSlug },
        body: { slug, name: `Scratch Project ${slug}` },
      });

      expect(response.response.status).toBe(201);
      expect(response.data?.slug).toBe(slug);
    });

    it("can create an org-scoped API key", async () => {
      const { client } = getContext();
      const response = await apiKeyCreate({
        client,
        body: {
          name: `Org Key ${scratchSuffix()}`,
          owner: { type: "organization", org_id: orgId },
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data?.key).toMatch(/^gw_/);
    });

    it("can create a user", async () => {
      const { client } = getContext();
      const externalId = `scratch-user-${scratchSuffix()}`;
      const response = await userCreate({
        client,
        body: {
          external_id: externalId,
          email: `${externalId}@example.com`,
          name: "Scratch User",
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data?.external_id).toBe(externalId);
    });

    // ── Team CRUD ─────────────────────────────────────────────────────────

    it("can create a team", async () => {
      const team = await createScratchTeam();
      expect(team.slug).toMatch(/^scratch-team-/);
      expect(team.id).toBeDefined();
    });

    it("can get a team by slug", async () => {
      const { client } = getContext();
      const team = await createScratchTeam();
      const response = await teamGet({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
      });

      expect(response.response.status).toBe(200);
      expect(response.data?.id).toBe(team.id);
    });

    it("can list teams in an organization", async () => {
      const { client } = getContext();
      const team = await createScratchTeam();
      const response = await teamList({
        client,
        path: { org_slug: orgSlug },
      });

      expect(response.response.status).toBe(200);
      expect(response.data?.data).toBeDefined();
      const teamSlugs = response.data?.data.map((t) => t.slug);
      expect(teamSlugs).toContain(team.slug);
    });

    it("can update a team", async () => {
      const { client } = getContext();
      const team = await createScratchTeam();
      const response = await teamUpdate({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
        body: { name: "Updated Team Name" },
      });

      expect(response.response.status).toBe(200);
      expect(response.data?.name).toBe("Updated Team Name");
    });

    it("can add a member to a team", async () => {
      const { client } = getContext();
      const team = await createScratchTeam();
      const response = await teamMemberAdd({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
        body: { user_id: userId, role: "member" },
      });

      expect(response.response.status).toBe(201);
      expect(response.data?.role).toBe("member");
    });

    it("can list team members", async () => {
      const { client } = getContext();
      const team = await createScratchTeam();
      await teamMemberAdd({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
        body: { user_id: userId, role: "member" },
      });
      const response = await teamMemberList({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
      });

      expect(response.response.status).toBe(200);
      const memberIds = response.data?.data.map((m) => m.user_id);
      expect(memberIds).toContain(userId);
    });

    it("can update a team member's role", async () => {
      const { client } = getContext();
      const team = await createScratchTeam();
      await teamMemberAdd({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
        body: { user_id: userId, role: "member" },
      });
      const response = await teamMemberUpdate({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug, user_id: userId },
        body: { role: "admin" },
      });

      expect(response.response.status).toBe(200);
      expect(response.data?.role).toBe("admin");
    });

    it("can remove a team member", async () => {
      const { client } = getContext();
      const team = await createScratchTeam();
      await teamMemberAdd({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
        body: { user_id: userId, role: "member" },
      });
      const response = await teamMemberRemove({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug, user_id: userId },
      });

      expect(response.response.status).toBe(200);

      const listResponse = await teamMemberList({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
      });
      const memberIds = listResponse.data?.data.map((m) => m.user_id);
      expect(memberIds).not.toContain(userId);
    });

    it("can create a team-scoped API key", async () => {
      const { client } = getContext();
      const team = await createScratchTeam();
      const response = await apiKeyCreate({
        client,
        body: {
          name: `Team API Key ${scratchSuffix()}`,
          owner: { type: "team", team_id: team.id },
        },
      });

      expect(response.response.status).toBe(201);
      expect(response.data?.key).toMatch(/^gw_/);
    });

    it("can delete a team", async () => {
      const { client } = getContext();
      const team = await createScratchTeam();
      const response = await teamDelete({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
      });

      expect(response.response.status).toBe(200);

      const getResponse = await teamGet({
        client,
        path: { org_slug: orgSlug, team_slug: team.slug },
      });
      expect(getResponse.response.status).toBe(404);
    });
  });
}
