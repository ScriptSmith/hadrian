/**
 * API Policy Enforcement Tests
 *
 * Tests API-level policy enforcement for model access, token limits, and feature gating.
 * Migrated from test_api_policy_enforcement() in deploy/test-e2e.sh.
 *
 * This module is auth-agnostic and works with both:
 * - OIDC (Bearer token authentication via Keycloak)
 * - SAML (Cookie-based session authentication via Authentik)
 *
 * Test scenarios (matching bash script exactly):
 *   1. Premium Model Access Control - Basic users blocked from premium models
 *   2. Token Limit Enforcement - Basic users limited to 1000 max_tokens
 *   3. Tools/Function Calling Feature Gate - Basic users blocked from using tools
 *   4. Reasoning Effort Control - Basic users blocked from high reasoning effort
 *   5. File Search (RAG) Feature Gate - Basic users blocked from file_search
 *
 * Test users:
 *   | Role        | Username   | Keycloak Roles                           |
 *   |-------------|------------|------------------------------------------|
 *   | premiumUser | prof_smith | premium, tools_enabled, rag_enabled      |
 *   | basicUser   | phd_bob    | user (no premium/tools/rag roles)        |
 *
 * Configured API Policies (from hadrian.university.toml):
 *   | Policy Name               | Condition                                         | Effect |
 *   |---------------------------|---------------------------------------------------|--------|
 *   | api-premium-model-access  | Model contains '/premium-' AND no 'premium' role  | deny   |
 *   | api-token-limit-basic     | max_tokens > 1000 AND no 'premium' role           | deny   |
 *   | api-tools-feature-gate    | has_tools AND no 'tools_enabled' role             | deny   |
 *   | api-file-search-gate      | has_file_search AND no 'rag_enabled' role         | deny   |
 *   | api-reasoning-premium     | reasoning_effort == 'high' AND no 'premium' role  | deny   |
 */
import { describe, it, expect } from "vitest";
import type { KeycloakTestContext } from "../../fixtures/keycloak";
import { trackedFetch } from "../../utils/tracked-fetch";

/**
 * Role types for API policy tests.
 * - premiumUser: Has premium, tools_enabled, rag_enabled roles (prof_smith)
 * - basicUser: Has only 'user' role (phd_bob)
 */
export type ApiPolicyTestRole = "premiumUser" | "basicUser";

/**
 * Auth-agnostic context for API policy enforcement tests.
 * Works with both OIDC tokens and SAML cookies.
 */
export interface ApiPolicyEnforcementContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /**
   * Get authentication headers for a specific role.
   * Returns either `Authorization: Bearer <token>` for OIDC
   * or `Cookie: __gw_session=<cookie>` for SAML.
   */
  getAuthHeaders: (role: ApiPolicyTestRole) => Record<string, string>;
}

/**
 * Create an API policy context from an OIDC/Keycloak test context.
 * Maps Keycloak tokens to auth headers.
 *
 * Role mapping:
 * - premiumUser → teamAdmin token (prof_smith with premium, tools_enabled, rag_enabled)
 * - basicUser → user token (phd_bob with just 'user' role)
 */
export function createOidcApiPolicyContext(
  gatewayUrl: string,
  keycloakContext: KeycloakTestContext,
): ApiPolicyEnforcementContext {
  return {
    gatewayUrl,
    getAuthHeaders: (role: ApiPolicyTestRole) => {
      const tokenMap = {
        // prof_smith (teamAdmin) has premium, tools_enabled, rag_enabled roles
        premiumUser: keycloakContext.tokens.teamAdmin.access_token,
        // phd_bob (user) has only 'user' role
        basicUser: keycloakContext.tokens.user.access_token,
      };
      return { Authorization: `Bearer ${tokenMap[role]}` };
    },
  };
}

/**
 * Create an API policy context from SAML session cookies.
 * Maps role names to session cookies.
 */
export function createSamlApiPolicyContext(
  gatewayUrl: string,
  sessionCookies: Record<ApiPolicyTestRole, string>,
): ApiPolicyEnforcementContext {
  return {
    gatewayUrl,
    getAuthHeaders: (role: ApiPolicyTestRole) => {
      return { Cookie: `__gw_session=${sessionCookies[role]}` };
    },
  };
}

/**
 * Run API policy enforcement tests.
 * Tests match test_api_policy_enforcement() from bash script exactly.
 *
 * @param getContext - Function that returns the test context
 */
export function runApiPolicyEnforcementTests(
  getContext: () => ApiPolicyEnforcementContext,
) {
  describe("API Policy Enforcement", () => {
    // =========================================================================
    // Test 1: Premium Model Access Control
    // =========================================================================
    describe("Premium Model Access Control", () => {
      it("basic user cannot use premium model", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("basicUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/premium-model",
              messages: [{ role: "user", content: "Hello" }],
            }),
          }
        );

        expect(response.status).toBe(403);
      });

      it("premium user can use premium model", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("premiumUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/premium-model",
              messages: [{ role: "user", content: "Hello" }],
            }),
          }
        );

        expect(response.status).toBe(200);
      });

      it("basic user can use regular model", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("basicUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello" }],
            }),
          }
        );

        expect(response.status).toBe(200);
      });
    });

    // =========================================================================
    // Test 2: Token Limit Enforcement
    // =========================================================================
    describe("Token Limit Enforcement", () => {
      it("basic user cannot request > 1000 tokens", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("basicUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello" }],
              max_tokens: 2000,
            }),
          }
        );

        expect(response.status).toBe(403);
      });

      it("premium user can request > 1000 tokens", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("premiumUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello" }],
              max_tokens: 2000,
            }),
          }
        );

        expect(response.status).toBe(200);
      });

      it("basic user can request <= 1000 tokens", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("basicUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello" }],
              max_tokens: 500,
            }),
          }
        );

        expect(response.status).toBe(200);
      });
    });

    // =========================================================================
    // Test 3: Tools/Function Calling Feature Gate
    // =========================================================================
    describe("Tools/Function Calling Feature Gate", () => {
      it("basic user cannot use tools", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("basicUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "What is the weather?" }],
              tools: [
                {
                  type: "function",
                  function: {
                    name: "get_weather",
                    parameters: {
                      type: "object",
                      properties: {
                        location: { type: "string" },
                      },
                    },
                  },
                },
              ],
            }),
          }
        );

        expect(response.status).toBe(403);
      });

      it("premium user can use tools", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("premiumUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "What is the weather?" }],
              tools: [
                {
                  type: "function",
                  function: {
                    name: "get_weather",
                    parameters: {
                      type: "object",
                      properties: {
                        location: { type: "string" },
                      },
                    },
                  },
                },
              ],
            }),
          }
        );

        expect(response.status).toBe(200);
      });
    });

    // =========================================================================
    // Test 4: Reasoning Effort Control
    // =========================================================================
    describe("Reasoning Effort Control", () => {
      it("basic user cannot use high reasoning effort", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("basicUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [
                {
                  role: "user",
                  content: "Think carefully about this problem.",
                },
              ],
              reasoning: { effort: "high" },
            }),
          }
        );

        expect(response.status).toBe(403);
      });

      it("premium user can use high reasoning effort", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("premiumUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [
                {
                  role: "user",
                  content: "Think carefully about this problem.",
                },
              ],
              reasoning: { effort: "high" },
            }),
          }
        );

        expect(response.status).toBe(200);
      });

      it("basic user can use low reasoning effort", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              ...getAuthHeaders("basicUser"),
              "Content-Type": "application/json",
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Think about this." }],
              reasoning: { effort: "low" },
            }),
          }
        );

        expect(response.status).toBe(200);
      });
    });

    // =========================================================================
    // Test 5: File Search (RAG) Feature Gate
    // =========================================================================
    describe("File Search (RAG) Feature Gate", () => {
      it("basic user cannot use file_search tool", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        // File search is tested via the Responses API
        const response = await trackedFetch(`${gatewayUrl}/api/v1/responses`, {
          method: "POST",
          headers: {
            ...getAuthHeaders("basicUser"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            model: "test/test-model",
            input: "Search the files for information about testing.",
            tools: [{ type: "file_search" }],
          }),
        });

        expect(response.status).toBe(403);
      });

      it("premium user can use file_search tool", async () => {
        const { gatewayUrl, getAuthHeaders } = getContext();

        // File search is tested via the Responses API
        const response = await trackedFetch(`${gatewayUrl}/api/v1/responses`, {
          method: "POST",
          headers: {
            ...getAuthHeaders("premiumUser"),
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            model: "test/test-model",
            input: "Search the files for information about testing.",
            tools: [{ type: "file_search" }],
          }),
        });

        expect(response.status).toBe(200);
      });
    });
  });
}
