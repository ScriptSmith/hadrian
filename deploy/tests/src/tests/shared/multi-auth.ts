/**
 * Multi-Auth Tests
 *
 * Tests multi-auth mode where both API keys and JWT tokens are supported.
 * This tests format-based detection and deterministic credential handling.
 *
 * Test scenarios:
 *   1. API key in X-API-Key header works
 *   2. API key in Bearer header works (format detection)
 *   3. JWT in Bearer header works
 *   4. Dual headers are handled deterministically (request proceeds)
 */
import { describe, it, expect } from "vitest";
import { trackedFetch } from "../../utils/tracked-fetch";

/**
 * Context for multi-auth tests.
 */
export interface MultiAuthContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /** API key for testing (gw_ prefixed) */
  apiKey: string;
  /** JWT token for testing */
  jwtToken: string;
}

/**
 * Run multi-auth tests.
 *
 * @param getContext - Function that returns the test context
 */
export function runMultiAuthTests(getContext: () => MultiAuthContext) {
  describe("Multi-Auth Mode", () => {
    // =========================================================================
    // API Key Authentication
    // =========================================================================
    describe("API Key Authentication", () => {
      it("API key in X-API-Key header works", async () => {
        const { gatewayUrl, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKey,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello" }],
            }),
          },
        );

        expect(response.status).toBe(200);
      });

      it("API key in Bearer header works (format detection)", async () => {
        const { gatewayUrl, apiKey } = getContext();

        // API key with gw_ prefix should be detected and validated
        // even when provided in the Authorization: Bearer header
        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${apiKey}`,
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

    // =========================================================================
    // JWT Authentication
    // =========================================================================
    describe("JWT Authentication", () => {
      it("JWT in Bearer header works", async () => {
        const { gatewayUrl, jwtToken } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${jwtToken}`,
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

    // =========================================================================
    // Ambiguous Credentials Handling
    // =========================================================================
    describe("Ambiguous Credentials", () => {
      it("dual headers rejected as ambiguous", async () => {
        const { gatewayUrl, apiKey, jwtToken } = getContext();

        // In multi-auth mode, providing both X-API-Key and Authorization
        // headers is rejected — clients should use one or the other.
        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKey,
              Authorization: `Bearer ${jwtToken}`,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello" }],
            }),
          },
        );

        expect(response.status).toBe(400);
        const body = await response.json();
        expect(body.error.code).toBe("ambiguous_credentials");
      });

      it("API key in both headers rejected as ambiguous", async () => {
        const { gatewayUrl, apiKey } = getContext();

        // Even when both headers contain the same API key, it's still ambiguous
        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKey,
              Authorization: `Bearer ${apiKey}`,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello" }],
            }),
          },
        );

        expect(response.status).toBe(400);
        const body = await response.json();
        expect(body.error.code).toBe("ambiguous_credentials");
      });
    });

    // =========================================================================
    // Bearer Prefix Case Insensitivity
    // =========================================================================
    describe("Bearer Prefix Handling", () => {
      it("bearer prefix is case-insensitive for API keys", async () => {
        const { gatewayUrl, apiKey } = getContext();

        // Test lowercase "bearer" prefix
        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `bearer ${apiKey}`,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "Hello" }],
            }),
          },
        );

        expect(response.status).toBe(200);
      });

      it("bearer prefix is case-insensitive for JWT", async () => {
        const { gatewayUrl, jwtToken } = getContext();

        // Test uppercase "BEARER" prefix
        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `BEARER ${jwtToken}`,
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
  });
}
