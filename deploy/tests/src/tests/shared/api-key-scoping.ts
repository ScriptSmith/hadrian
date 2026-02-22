/**
 * API Key Scoping Tests
 *
 * Tests API key scoping features including permission scopes, model restrictions,
 * key rotation with grace period, and per-key rate limits.
 * Migrated from test_api_key_scoping() in deploy/test-e2e.sh.
 *
 * Test scenarios (matching bash script exactly):
 *   1. Permission Scope Enforcement - Chat-only keys blocked from models endpoint
 *   2. Model Restrictions - Keys restricted to `test/*` blocked from other models
 *   3. Key Rotation with Grace Period - Both old and new keys work during grace
 *   4. Per-Key Rate Limits - Custom rate limits correctly set on creation
 */
import { describe, it, expect } from "vitest";
import { trackedFetch } from "../../utils/tracked-fetch";

/**
 * Context for API key scoping tests.
 */
export interface ApiKeyScopingContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /** Bearer token for admin API access */
  adminToken: string;
  /** Organization ID for API key ownership */
  orgId: string;
}

/**
 * Run API key scoping tests.
 * Tests match test_api_key_scoping() from bash script.
 *
 * @param getContext - Function that returns the test context
 */
export function runApiKeyScopingTests(getContext: () => ApiKeyScopingContext) {
  describe("API Key Scoping", () => {
    // =========================================================================
    // Test 1: Permission Scope Enforcement
    // =========================================================================
    describe("Permission Scope Enforcement", () => {
      it("creates a chat-only scoped key with correct scopes", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        const response = await trackedFetch(`${gatewayUrl}/admin/v1/api-keys`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${adminToken}`,
          },
          body: JSON.stringify({
            name: "Chat Only Key",
            owner: { type: "organization", org_id: orgId },
            scopes: ["chat"],
          }),
        });

        expect(response.status).toBe(201);
        const data = await response.json();
        expect(data.key).toMatch(/^gw_/);
        expect(data.scopes).toEqual(["chat"]);
      });

      it("chat-only key can access chat endpoint", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        // Create chat-only key
        const createResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({
              name: "Chat Only Key - Access Test",
              owner: { type: "organization", org_id: orgId },
              scopes: ["chat"],
            }),
          },
        );

        expect(createResponse.status).toBe(201);
        const keyData = await createResponse.json();
        const scopedKey = keyData.key;

        // Verify chat endpoint works
        const chatResponse = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": scopedKey,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "test" }],
            }),
          },
        );

        expect(chatResponse.status).toBe(200);
      });

      it("chat-only key is blocked from models endpoint", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        // Create chat-only key
        const createResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({
              name: "Chat Only Key - Block Test",
              owner: { type: "organization", org_id: orgId },
              scopes: ["chat"],
            }),
          },
        );

        expect(createResponse.status).toBe(201);
        const keyData = await createResponse.json();
        const scopedKey = keyData.key;

        // Verify models endpoint is blocked
        const modelsResponse = await trackedFetch(
          `${gatewayUrl}/api/v1/models`,
          {
            headers: {
              "X-API-Key": scopedKey,
            },
          },
        );

        expect(modelsResponse.status).toBe(403);
      });
    });

    // =========================================================================
    // Test 2: Model Restrictions
    // =========================================================================
    describe("Model Restrictions", () => {
      it("creates a model-restricted key with correct allowed_models", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        const response = await trackedFetch(`${gatewayUrl}/admin/v1/api-keys`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${adminToken}`,
          },
          body: JSON.stringify({
            name: "Test Models Only Key",
            owner: { type: "organization", org_id: orgId },
            allowed_models: ["test/*"],
          }),
        });

        expect(response.status).toBe(201);
        const data = await response.json();
        expect(data.key).toMatch(/^gw_/);
        expect(data.allowed_models).toEqual(["test/*"]);
      });

      it("model-restricted key can access allowed model", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        // Create model-restricted key
        const createResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({
              name: "Test Models Key - Access Test",
              owner: { type: "organization", org_id: orgId },
              allowed_models: ["test/*"],
            }),
          },
        );

        expect(createResponse.status).toBe(201);
        const keyData = await createResponse.json();
        const modelKey = keyData.key;

        // Verify test/test-model works
        const chatResponse = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": modelKey,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "test" }],
            }),
          },
        );

        expect(chatResponse.status).toBe(200);
      });

      it("model-restricted key is blocked from non-allowed model", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        // Create model-restricted key
        const createResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({
              name: "Test Models Key - Block Test",
              owner: { type: "organization", org_id: orgId },
              allowed_models: ["test/*"],
            }),
          },
        );

        expect(createResponse.status).toBe(201);
        const keyData = await createResponse.json();
        const modelKey = keyData.key;

        // Verify gpt-4 is blocked (403 for model restriction)
        const chatResponse = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": modelKey,
            },
            body: JSON.stringify({
              model: "gpt-4",
              messages: [{ role: "user", content: "test" }],
            }),
          },
        );

        // Model restriction check returns 403
        expect(chatResponse.status).toBe(403);
      });
    });

    // =========================================================================
    // Test 3: Key Rotation with Grace Period
    // =========================================================================
    describe("Key Rotation with Grace Period", () => {
      it("rotated key has rotation_grace_until set", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        // Create a key to rotate
        const createResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({
              name: "Key for Rotation Test",
              owner: { type: "organization", org_id: orgId },
            }),
          },
        );

        expect(createResponse.status).toBe(201);
        const keyData = await createResponse.json();
        const keyId = keyData.id;

        // Rotate with 1 hour grace period
        const rotateResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys/${keyId}/rotate`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({ grace_period_seconds: 3600 }),
          },
        );

        expect(rotateResponse.status).toBe(201);
        const rotatedData = await rotateResponse.json();
        expect(rotatedData.key).toMatch(/^gw_/);
        expect(rotatedData.rotation_grace_until).toBeDefined();
      });

      it("original key works during grace period", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        // Create a key to rotate
        const createResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({
              name: "Key for Rotation - Old Key Test",
              owner: { type: "organization", org_id: orgId },
            }),
          },
        );

        expect(createResponse.status).toBe(201);
        const keyData = await createResponse.json();
        const originalKey = keyData.key;
        const keyId = keyData.id;

        // Rotate with 1 hour grace period
        const rotateResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys/${keyId}/rotate`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({ grace_period_seconds: 3600 }),
          },
        );

        expect(rotateResponse.status).toBe(201);

        // Verify original key still works during grace period
        const chatResponse = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": originalKey,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "test" }],
            }),
          },
        );

        expect(chatResponse.status).toBe(200);
      });

      it("new key works after rotation", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        // Create a key to rotate
        const createResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({
              name: "Key for Rotation - New Key Test",
              owner: { type: "organization", org_id: orgId },
            }),
          },
        );

        expect(createResponse.status).toBe(201);
        const keyData = await createResponse.json();
        const keyId = keyData.id;

        // Rotate with 1 hour grace period
        const rotateResponse = await trackedFetch(
          `${gatewayUrl}/admin/v1/api-keys/${keyId}/rotate`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              Authorization: `Bearer ${adminToken}`,
            },
            body: JSON.stringify({ grace_period_seconds: 3600 }),
          },
        );

        expect(rotateResponse.status).toBe(201);
        const rotatedData = await rotateResponse.json();
        const newKey = rotatedData.key;

        // Verify new key works
        const chatResponse = await trackedFetch(
          `${gatewayUrl}/api/v1/chat/completions`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": newKey,
            },
            body: JSON.stringify({
              model: "test/test-model",
              messages: [{ role: "user", content: "test" }],
            }),
          },
        );

        expect(chatResponse.status).toBe(200);
      });
    });

    // =========================================================================
    // Test 4: Per-Key Rate Limits
    // =========================================================================
    describe("Per-Key Rate Limits", () => {
      it("creates a key with custom rate limits", async () => {
        const { gatewayUrl, adminToken, orgId } = getContext();

        // Create a key with custom rate limits (must be <= global defaults)
        const response = await trackedFetch(`${gatewayUrl}/admin/v1/api-keys`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${adminToken}`,
          },
          body: JSON.stringify({
            name: "Rate Limited Key",
            owner: { type: "organization", org_id: orgId },
            rate_limit_rpm: 30,
            rate_limit_tpm: 50000,
          }),
        });

        expect(response.status).toBe(201);
        const data = await response.json();
        expect(data.key).toMatch(/^gw_/);
        expect(data.rate_limit_rpm).toBe(30);
        expect(data.rate_limit_tpm).toBe(50000);
      });
    });

    // Note: IP allowlist tests are skipped because Docker networking makes it
    // difficult to test IP-based restrictions reliably. IP allowlist validation
    // is covered by unit tests in src/models/api_key.rs.
  });
}
