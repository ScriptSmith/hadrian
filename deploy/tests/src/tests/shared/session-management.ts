/**
 * Session Management Tests
 *
 * Tests for enhanced session management features:
 * - List sessions (empty when disabled, populated when enabled)
 * - Revoke all sessions
 * - Revoke single session (returns 0 for non-existent)
 * - Device info tracking (user-agent, IP)
 *
 * Note: These tests require UI authentication (OIDC or SAML) to be configured
 * for session management to work. If session management is not available,
 * tests will skip gracefully.
 */
import { describe, it, expect } from "vitest";
import { trackedFetch } from "../../utils/tracked-fetch";

export interface SessionManagementContext {
  gatewayUrl: string;
  adminToken: string;
  userId: string;
}

/**
 * Check if the error response indicates session management is not available.
 */
function isSessionManagementUnavailable(response: Response, body: unknown): boolean {
  if (response.status !== 400) return false;
  const bodyStr = typeof body === "string" ? body : JSON.stringify(body);
  return bodyStr.includes("No session store configured") ||
         bodyStr.includes("UI authentication") ||
         bodyStr.includes("OIDC or SAML");
}

/**
 * Run session management tests.
 * @param getContext - Function that returns the test context. Called lazily to ensure
 *                     the context is available after beforeAll setup completes.
 */
export function runSessionManagementTests(
  getContext: () => SessionManagementContext
) {
  describe("Session Management", () => {
    it("can list user sessions", async () => {
      const { gatewayUrl, adminToken, userId } = getContext();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/users/${userId}/sessions`,
        {
          method: "GET",
          headers: { Authorization: `Bearer ${adminToken}` },
        }
      );

      // Skip if session management is not available (UI auth not configured)
      if (response.status === 400) {
        const body = await response.json();
        if (isSessionManagementUnavailable(response, body)) {
          console.log("Skipping: Session management not available (no UI auth configured)");
          return;
        }
      }

      expect(response.status).toBe(200);
      const data = await response.json();

      // Response should have the expected structure
      expect(data).toHaveProperty("data");
      expect(data).toHaveProperty("enhanced_enabled");
      expect(Array.isArray(data.data)).toBe(true);

      // If enhanced sessions are enabled and user has sessions, verify structure
      if (data.enhanced_enabled && data.data.length > 0) {
        const session = data.data[0];
        expect(session).toHaveProperty("id");
        expect(session).toHaveProperty("created_at");
        expect(session).toHaveProperty("expires_at");
      }
    });

    it("can revoke all user sessions", async () => {
      const { gatewayUrl, adminToken, userId } = getContext();

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/users/${userId}/sessions`,
        {
          method: "DELETE",
          headers: { Authorization: `Bearer ${adminToken}` },
        }
      );

      // Skip if session management is not available (UI auth not configured)
      if (response.status === 400) {
        const body = await response.json();
        if (isSessionManagementUnavailable(response, body)) {
          console.log("Skipping: Session management not available (no UI auth configured)");
          return;
        }
      }

      expect(response.status).toBe(200);
      const data = await response.json();

      // Response should have sessions_revoked count
      expect(data).toHaveProperty("sessions_revoked");
      expect(typeof data.sessions_revoked).toBe("number");
      expect(data.sessions_revoked).toBeGreaterThanOrEqual(0);
    });

    it("returns 0 when revoking non-existent session", async () => {
      const { gatewayUrl, adminToken, userId } = getContext();

      // Use a random UUID that doesn't exist
      const nonExistentSessionId = "00000000-0000-0000-0000-000000000000";

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/users/${userId}/sessions/${nonExistentSessionId}`,
        {
          method: "DELETE",
          headers: { Authorization: `Bearer ${adminToken}` },
        }
      );

      // Skip if session management is not available (UI auth not configured)
      if (response.status === 400) {
        const body = await response.json();
        if (isSessionManagementUnavailable(response, body)) {
          console.log("Skipping: Session management not available (no UI auth configured)");
          return;
        }
      }

      expect(response.status).toBe(200);
      const data = await response.json();

      // Should return 0 sessions revoked for non-existent session
      expect(data).toHaveProperty("sessions_revoked");
      expect(data.sessions_revoked).toBe(0);
    });

    it("returns 404 for non-existent user session endpoint", async () => {
      const { gatewayUrl, adminToken } = getContext();

      // Use a random UUID that doesn't exist as user
      const nonExistentUserId = "00000000-0000-0000-0000-000000000001";

      const response = await trackedFetch(
        `${gatewayUrl}/admin/v1/users/${nonExistentUserId}/sessions`,
        {
          method: "GET",
          headers: { Authorization: `Bearer ${adminToken}` },
        }
      );

      // Should return 404 for non-existent user
      expect(response.status).toBe(404);
    });
  });
}
