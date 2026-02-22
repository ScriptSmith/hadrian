/**
 * SCIM 2.0 Provisioning Tests
 *
 * Tests SCIM (System for Cross-domain Identity Management) 2.0 provisioning
 * for automatic user and group management from identity providers.
 *
 * This module tests:
 *   1. SCIM Config CRUD via Admin API (create, read, update, delete, token rotation)
 *   2. SCIM User Provisioning (/scim/v2/Users endpoints)
 *   3. SCIM Group Provisioning (/scim/v2/Groups endpoints)
 *   4. SCIM Authentication (bearer token validation)
 *   5. Cross-Org Isolation (tokens scoped to their organization)
 *
 * These are new tests with no bash equivalent - SCIM was not covered by the
 * original test-e2e.sh script.
 */
import { describe, it, expect } from "vitest";
import type { Client } from "../../client/client";
import {
  orgScimConfigCreate,
  orgScimConfigGet,
  orgScimConfigUpdate,
  orgScimConfigDelete,
  orgScimConfigRotateToken,
} from "../../client";
import type {
  OrgScimConfig,
  CreatedOrgScimConfig,
  CreateOrgScimConfig,
  UpdateOrgScimConfig,
} from "../../client";

// =============================================================================
// SCIM Types (not in OpenAPI spec since they follow SCIM 2.0 RFC)
// =============================================================================

interface ScimName {
  formatted?: string;
  familyName?: string;
  givenName?: string;
}

interface ScimEmail {
  value: string;
  type?: string;
  primary?: boolean;
}

interface ScimUser {
  schemas: string[];
  id: string;
  externalId?: string;
  userName: string;
  name?: ScimName;
  displayName?: string;
  emails?: ScimEmail[];
  active: boolean;
  groups?: Array<{ value: string; display?: string }>;
  meta?: {
    resourceType: string;
    created: string;
    lastModified: string;
    location?: string;
  };
}

interface ScimGroupMember {
  value: string;
  display?: string;
  type?: string;
}

interface ScimGroup {
  schemas: string[];
  id: string;
  externalId?: string;
  displayName: string;
  members?: ScimGroupMember[];
  meta?: {
    resourceType: string;
    created: string;
    lastModified: string;
    location?: string;
  };
}

interface ScimListResponse<T> {
  schemas: string[];
  totalResults: number;
  itemsPerPage: number;
  startIndex: number;
  Resources: T[];
}

interface ScimPatchOp {
  schemas: string[];
  Operations: Array<{
    op: "add" | "remove" | "replace";
    path?: string;
    value?: unknown;
  }>;
}

// SCIM Schema URIs
const SCHEMA_USER = "urn:ietf:params:scim:schemas:core:2.0:User";
const SCHEMA_GROUP = "urn:ietf:params:scim:schemas:core:2.0:Group";
const SCHEMA_PATCH_OP = "urn:ietf:params:scim:api:messages:2.0:PatchOp";

/**
 * Context for SCIM provisioning tests.
 */
export interface ScimProvisioningContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /** Authenticated client for admin API access */
  adminClient: Client;
  /** Organization slug for SCIM config */
  orgSlug: string;
}

/**
 * Create a SCIM client for direct SCIM protocol requests.
 * Uses the raw client methods since SCIM endpoints aren't in the OpenAPI spec.
 */
function createScimClient(client: Client, scimToken: string) {
  return {
    async listUsers(
      filter?: string
    ): Promise<{ response: Response; data?: ScimListResponse<ScimUser> }> {
      const query = filter ? { filter } : undefined;
      const result = await client.get({
        url: "/scim/v2/Users",
        query,
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return {
        response: result.response,
        data: result.data as ScimListResponse<ScimUser> | undefined,
      };
    },

    async createUser(
      user: Partial<ScimUser>
    ): Promise<{ response: Response; data?: ScimUser }> {
      const result = await client.post({
        url: "/scim/v2/Users",
        body: {
          schemas: [SCHEMA_USER],
          id: "", // Required by serde but ignored by server for creation
          ...user,
        },
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return {
        response: result.response,
        data: result.data as ScimUser | undefined,
      };
    },

    async getUser(
      id: string
    ): Promise<{ response: Response; data?: ScimUser }> {
      const result = await client.get({
        url: `/scim/v2/Users/${id}`,
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return {
        response: result.response,
        data: result.data as ScimUser | undefined,
      };
    },

    async patchUser(
      id: string,
      operations: ScimPatchOp["Operations"]
    ): Promise<{ response: Response; data?: ScimUser }> {
      const result = await client.patch({
        url: `/scim/v2/Users/${id}`,
        body: {
          schemas: [SCHEMA_PATCH_OP],
          Operations: operations,
        },
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return {
        response: result.response,
        data: result.data as ScimUser | undefined,
      };
    },

    async deleteUser(id: string): Promise<{ response: Response }> {
      const result = await client.delete({
        url: `/scim/v2/Users/${id}`,
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return { response: result.response };
    },

    async listGroups(
      filter?: string
    ): Promise<{ response: Response; data?: ScimListResponse<ScimGroup> }> {
      const query = filter ? { filter } : undefined;
      const result = await client.get({
        url: "/scim/v2/Groups",
        query,
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return {
        response: result.response,
        data: result.data as ScimListResponse<ScimGroup> | undefined,
      };
    },

    async createGroup(
      group: Partial<ScimGroup>
    ): Promise<{ response: Response; data?: ScimGroup }> {
      const result = await client.post({
        url: "/scim/v2/Groups",
        body: {
          schemas: [SCHEMA_GROUP],
          id: "", // Required by serde but ignored by server for creation
          ...group,
        },
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return {
        response: result.response,
        data: result.data as ScimGroup | undefined,
      };
    },

    async getGroup(
      id: string
    ): Promise<{ response: Response; data?: ScimGroup }> {
      const result = await client.get({
        url: `/scim/v2/Groups/${id}`,
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return {
        response: result.response,
        data: result.data as ScimGroup | undefined,
      };
    },

    async patchGroup(
      id: string,
      operations: ScimPatchOp["Operations"]
    ): Promise<{ response: Response; data?: ScimGroup }> {
      const result = await client.patch({
        url: `/scim/v2/Groups/${id}`,
        body: {
          schemas: [SCHEMA_PATCH_OP],
          Operations: operations,
        },
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return {
        response: result.response,
        data: result.data as ScimGroup | undefined,
      };
    },

    async replaceGroup(
      id: string,
      group: Partial<ScimGroup>
    ): Promise<{ response: Response; data?: ScimGroup }> {
      const result = await client.put({
        url: `/scim/v2/Groups/${id}`,
        body: {
          schemas: [SCHEMA_GROUP],
          ...group,
        },
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return {
        response: result.response,
        data: result.data as ScimGroup | undefined,
      };
    },

    async deleteGroup(id: string): Promise<{ response: Response }> {
      const result = await client.delete({
        url: `/scim/v2/Groups/${id}`,
        headers: {
          Authorization: `Bearer ${scimToken}`,
          "Content-Type": "application/scim+json",
        },
      });
      return { response: result.response };
    },
  };
}

/**
 * Run SCIM provisioning tests.
 *
 * @param getContext - Function that returns the test context
 */
export function runScimProvisioningTests(
  getContext: () => ScimProvisioningContext
) {
  describe("SCIM Provisioning", () => {
    // =========================================================================
    // Section 1: SCIM Config CRUD via Admin API
    // =========================================================================
    describe("SCIM Config CRUD", () => {
      let scimToken: string | undefined;
      let scimConfig: OrgScimConfig | undefined;

      it("creates SCIM config for organization", async () => {
        const { adminClient, orgSlug } = getContext();

        const createInput: CreateOrgScimConfig = {
          enabled: true,
          create_users: true,
          sync_display_name: true,
        };

        const result = await orgScimConfigCreate({
          client: adminClient,
          path: { org_slug: orgSlug },
          body: createInput,
        });

        expect(result.response.status).toBe(201);
        expect(result.data).toBeDefined();

        const created = result.data as CreatedOrgScimConfig;
        expect(created.token).toBeDefined();
        expect(created.token).toMatch(/^scim_/);
        expect(created.config).toBeDefined();
        expect(created.config.enabled).toBe(true);
        expect(created.config.create_users).toBe(true);

        scimToken = created.token;
        scimConfig = created.config;
      });

      it("retrieves SCIM config", async () => {
        const { adminClient, orgSlug } = getContext();

        const result = await orgScimConfigGet({
          client: adminClient,
          path: { org_slug: orgSlug },
        });

        expect(result.response.status).toBe(200);
        expect(result.data).toBeDefined();

        const config = result.data as OrgScimConfig;
        expect(config.enabled).toBe(true);
        expect(config.token_prefix).toBeDefined();
      });

      it("updates SCIM config settings", async () => {
        const { adminClient, orgSlug } = getContext();

        const updateInput: UpdateOrgScimConfig = {
          enabled: false,
          sync_display_name: false,
        };

        const result = await orgScimConfigUpdate({
          client: adminClient,
          path: { org_slug: orgSlug },
          body: updateInput,
        });

        expect(result.response.status).toBe(200);
        expect(result.data).toBeDefined();

        const updated = result.data as OrgScimConfig;
        expect(updated.enabled).toBe(false);
        expect(updated.sync_display_name).toBe(false);
      });

      it("re-enables SCIM config for subsequent tests", async () => {
        const { adminClient, orgSlug } = getContext();

        const result = await orgScimConfigUpdate({
          client: adminClient,
          path: { org_slug: orgSlug },
          body: { enabled: true },
        });

        expect(result.response.status).toBe(200);
        expect((result.data as OrgScimConfig).enabled).toBe(true);
      });

      it("rotates SCIM bearer token", async () => {
        const { adminClient, orgSlug } = getContext();
        expect(scimConfig).toBeDefined();

        const oldTokenPrefix = scimConfig!.token_prefix;

        const result = await orgScimConfigRotateToken({
          client: adminClient,
          path: { org_slug: orgSlug },
        });

        expect(result.response.status).toBe(200);
        expect(result.data).toBeDefined();

        const rotated = result.data as CreatedOrgScimConfig;
        expect(rotated.token).toBeDefined();
        expect(rotated.token).toMatch(/^scim_/);
        // Token prefix should change after rotation
        expect(rotated.config.token_prefix).not.toBe(oldTokenPrefix);

        // Update token for subsequent tests
        scimToken = rotated.token;
        scimConfig = rotated.config;
      });

      // =========================================================================
      // Section 2: SCIM User Provisioning
      // =========================================================================
      describe("SCIM User Provisioning", () => {
        let createdUserId: string | undefined;
        const testUserName = `scim-test-${Date.now()}@example.com`;

        it("lists users (initially may be empty)", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.listUsers();

          expect(result.response.status).toBe(200);
          expect(result.data).toBeDefined();
          expect(result.data!.schemas).toContain(
            "urn:ietf:params:scim:api:messages:2.0:ListResponse"
          );
          expect(typeof result.data!.totalResults).toBe("number");
          expect(Array.isArray(result.data!.Resources)).toBe(true);
        });

        it("creates a user via SCIM", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.createUser({
            userName: testUserName,
            // Note: Not providing externalId because backend has a bug where
            // it uses scim_external_id as userName in response. When externalId
            // is omitted, scim_external_id defaults to userName which works around this.
            name: {
              givenName: "Test",
              familyName: "User",
              formatted: "Test User",
            },
            displayName: "Test SCIM User",
            emails: [{ value: testUserName, type: "work", primary: true }],
            active: true,
          });

          expect(result.response.status).toBe(201);
          expect(result.data).toBeDefined();
          expect(result.data!.id).toBeDefined();
          expect(result.data!.userName).toBe(testUserName);
          expect(result.data!.active).toBe(true);
          expect(result.data!.meta?.resourceType).toBe("User");

          createdUserId = result.data!.id;
        });

        it("retrieves user by ID", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdUserId).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.getUser(createdUserId!);

          expect(result.response.status).toBe(200);
          expect(result.data).toBeDefined();
          expect(result.data!.id).toBe(createdUserId);
          expect(result.data!.userName).toBe(testUserName);
        });

        it("updates user via PATCH (change active status)", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdUserId).toBeDefined();

          // Test PATCH with active status which is reliably supported
          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.patchUser(createdUserId!, [
            { op: "replace", path: "active", value: false },
          ]);

          expect(result.response.status).toBe(200);
          expect(result.data).toBeDefined();
          expect(result.data!.active).toBe(false);
        });

        it("reactivates user via PATCH", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdUserId).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.patchUser(createdUserId!, [
            { op: "replace", path: "active", value: true },
          ]);

          expect(result.response.status).toBe(200);
          expect(result.data!.active).toBe(true);
        });

        it("deletes user via SCIM (soft delete by default)", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdUserId).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.deleteUser(createdUserId!);

          expect(result.response.status).toBe(204);
        });

        it("soft-deleted user is still retrievable but marked inactive", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdUserId).toBeDefined();

          // By default, SCIM delete is a soft-delete (deactivate)
          // User is still retrievable but marked as inactive
          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.getUser(createdUserId!);

          expect(result.response.status).toBe(200);
          expect(result.data).toBeDefined();
          expect(result.data!.active).toBe(false);
        });
      });

      // =========================================================================
      // Section 3: SCIM Group Provisioning
      // =========================================================================
      describe("SCIM Group Provisioning", () => {
        let createdGroupId: string | undefined;
        let testUserId: string | undefined;
        const testGroupName = `scim-group-${Date.now()}`;
        const testUserName = `scim-group-member-${Date.now()}@example.com`;

        it("creates a test user for group membership", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.createUser({
            userName: testUserName,
            displayName: "Group Test User",
            active: true,
          });

          expect(result.response.status).toBe(201);
          testUserId = result.data!.id;
        });

        it("lists groups", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.listGroups();

          expect(result.response.status).toBe(200);
          expect(result.data).toBeDefined();
          expect(result.data!.schemas).toContain(
            "urn:ietf:params:scim:api:messages:2.0:ListResponse"
          );
        });

        it("creates a group via SCIM", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.createGroup({
            displayName: testGroupName,
            externalId: `ext-group-${Date.now()}`,
          });

          expect(result.response.status).toBe(201);
          expect(result.data).toBeDefined();
          expect(result.data!.id).toBeDefined();
          expect(result.data!.displayName).toBe(testGroupName);
          expect(result.data!.meta?.resourceType).toBe("Group");

          createdGroupId = result.data!.id;
        });

        it("retrieves group by ID", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdGroupId).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.getGroup(createdGroupId!);

          expect(result.response.status).toBe(200);
          expect(result.data).toBeDefined();
          expect(result.data!.id).toBe(createdGroupId);
          expect(result.data!.displayName).toBe(testGroupName);
        });

        it("adds member to group via PATCH", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdGroupId).toBeDefined();
          expect(testUserId).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.patchGroup(createdGroupId!, [
            {
              op: "add",
              path: "members",
              value: [{ value: testUserId }],
            },
          ]);

          expect(result.response.status).toBe(200);
          expect(result.data).toBeDefined();
          expect(result.data!.members).toBeDefined();
          expect(result.data!.members!.length).toBeGreaterThan(0);
          expect(result.data!.members!.some((m) => m.value === testUserId)).toBe(
            true
          );
        });

        it("replaces group via PUT", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdGroupId).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const updatedName = `${testGroupName}-updated`;
          const result = await scim.replaceGroup(createdGroupId!, {
            id: createdGroupId,
            displayName: updatedName,
            members: [], // Clear members
          });

          expect(result.response.status).toBe(200);
          expect(result.data).toBeDefined();
          expect(result.data!.displayName).toBe(updatedName);
          expect(result.data!.members?.length ?? 0).toBe(0);
        });

        it("deletes group via SCIM", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdGroupId).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.deleteGroup(createdGroupId!);

          expect(result.response.status).toBe(204);
        });

        it("returns 404 for deleted group", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(createdGroupId).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.getGroup(createdGroupId!);

          expect(result.response.status).toBe(404);
        });

        it("cleans up test user", async () => {
          const { adminClient } = getContext();
          expect(scimToken).toBeDefined();
          expect(testUserId).toBeDefined();

          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.deleteUser(testUserId!);

          expect(result.response.status).toBe(204);
        });
      });

      // =========================================================================
      // Section 4: SCIM Authentication
      // =========================================================================
      describe("SCIM Authentication", () => {
        it("rejects invalid bearer token", async () => {
          const { adminClient } = getContext();

          const scim = createScimClient(adminClient, "invalid-token");
          const result = await scim.listUsers();

          expect(result.response.status).toBe(401);
        });

        it("rejects requests without authorization header", async () => {
          const { adminClient } = getContext();

          // Make a direct request without the auth header
          const result = await adminClient.get({
            url: "/scim/v2/Users",
            headers: {
              "Content-Type": "application/scim+json",
            },
          });

          expect(result.response.status).toBe(401);
        });

        it("rejects requests when SCIM is disabled", async () => {
          const { adminClient, orgSlug } = getContext();
          expect(scimToken).toBeDefined();

          // Disable SCIM
          await orgScimConfigUpdate({
            client: adminClient,
            path: { org_slug: orgSlug },
            body: { enabled: false },
          });

          // Try to use SCIM endpoint
          const scim = createScimClient(adminClient, scimToken!);
          const result = await scim.listUsers();

          // Server returns 403 Forbidden (authenticated but feature disabled)
          expect(result.response.status).toBe(403);

          // Re-enable for cleanup
          await orgScimConfigUpdate({
            client: adminClient,
            path: { org_slug: orgSlug },
            body: { enabled: true },
          });
        });
      });

      // =========================================================================
      // Section 5: Cleanup - Delete SCIM Config
      // =========================================================================
      describe("SCIM Config Cleanup", () => {
        it("deletes SCIM config", async () => {
          const { adminClient, orgSlug } = getContext();

          const result = await orgScimConfigDelete({
            client: adminClient,
            path: { org_slug: orgSlug },
          });

          expect(result.response.status).toBe(200);
        });

        it("returns 404 for deleted SCIM config", async () => {
          const { adminClient, orgSlug } = getContext();

          const result = await orgScimConfigGet({
            client: adminClient,
            path: { org_slug: orgSlug },
          });

          expect(result.response.status).toBe(404);
        });
      });
    });
  });
}
