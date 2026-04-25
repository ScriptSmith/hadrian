/**
 * OAuth PKCE flow tests.
 *
 * Drives the same PKCE flow that an external app (e.g. `oauth-test/`) would
 * run, but skips the browser entirely: the test plays the role of the
 * external app, generates the verifier/challenge in Node, and POSTs to
 * `/admin/v1/oauth/authorize` (what the consent page does after "Allow")
 * and `/oauth/token` (what the redirected callback does).
 *
 * Covers:
 *  - RFC 8414 discovery document
 *  - Preflight callback validation (allow + deny legs)
 *  - Happy path: issue code → exchange for key → use key against /api/v1/chat/completions
 *  - Wrong verifier → invalid_grant
 *  - Code reuse → invalid_grant (single-use)
 *  - Non-loopback http callback rejected at authorize time
 *  - `code_challenge_method = "plain"` rejected when not enabled
 */
import { describe, it, expect } from "vitest";
import { createHash, randomBytes } from "node:crypto";
import type { Client } from "../../client/client";
import {
  oauthAuthorizationServerMetadata,
  oauthAuthorize,
  oauthPreflight,
  oauthToken,
  apiV1ChatCompletions,
} from "../../client";

export interface OAuthPkceContext {
  url: string;
  client: Client;
  apiKeyClient: (apiKey: string) => Client;
}

function base64url(buf: Buffer): string {
  return buf
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

/** Generate a fresh PKCE verifier/challenge pair (S256). */
function makePkcePair(): { verifier: string; challenge: string } {
  const verifier = base64url(randomBytes(32));
  const challenge = base64url(createHash("sha256").update(verifier).digest());
  return { verifier, challenge };
}

/**
 * Run OAuth PKCE flow tests.
 * @param getContext - lazy context accessor, evaluated after `beforeAll`.
 */
export function runOAuthPkceTests(getContext: () => OAuthPkceContext) {
  describe("OAuth PKCE flow", () => {
    // Loopback host so http is permitted by validate_callback_url.
    const callbackUrl = "http://localhost:9999/callback";

    it("publishes RFC 8414 authorization-server metadata", async () => {
      const { client } = getContext();
      const response = await oauthAuthorizationServerMetadata({ client });

      expect(response.response.status).toBe(200);
      const meta = response.data!;
      expect(meta.issuer).toBeTruthy();
      expect(meta.authorization_endpoint).toMatch(/\/oauth\/authorize$/);
      expect(meta.token_endpoint).toMatch(/\/oauth\/token$/);
      expect(meta.code_challenge_methods_supported).toContain("S256");
      expect(meta.code_challenge_methods_supported).not.toContain("plain");
      expect(meta.grant_types_supported).toEqual(["authorization_code"]);
      expect(meta.response_types_supported).toEqual(["code"]);
      expect(meta.token_endpoint_auth_methods_supported).toEqual(["none"]);
    });

    it("preflight accepts a loopback callback", async () => {
      const { client } = getContext();
      const response = await oauthPreflight({
        client,
        query: { callback_url: callbackUrl },
      });

      expect(response.response.status).toBe(200);
      expect(response.data?.callback_host).toBe("localhost");
    });

    it("preflight rejects http on a non-loopback host", async () => {
      const { client } = getContext();
      const response = await oauthPreflight({
        client,
        query: { callback_url: "http://example.com/callback" },
      });

      expect(response.response.status).toBe(400);
    });

    it("issues a code, exchanges it for a key, and uses the key", async () => {
      const { client, apiKeyClient } = getContext();
      const { verifier, challenge } = makePkcePair();

      const authResp = await oauthAuthorize({
        client,
        body: {
          callback_url: callbackUrl,
          code_challenge: challenge,
          code_challenge_method: "S256",
          app_name: "E2E PKCE test",
          key_options: { name: "OAuth-issued key (happy path)" },
        },
      });
      expect(authResp.response.status).toBe(201);
      const code = authResp.data!.code;
      expect(code).toBeTruthy();
      expect(authResp.data!.redirect_url).toContain(`code=${code}`);
      expect(authResp.data!.redirect_url.startsWith(callbackUrl)).toBe(true);
      expect(new Date(authResp.data!.expires_at).getTime()).toBeGreaterThan(
        Date.now()
      );

      const tokenResp = await oauthToken({
        client,
        body: {
          code,
          code_verifier: verifier,
          code_challenge_method: "S256",
        },
      });
      expect(tokenResp.response.status).toBe(200);
      const issued = tokenResp.data!;
      expect(issued.key).toMatch(/^gw_/);
      expect(issued.key_prefix).toBeTruthy();
      expect(issued.key_id).toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
      );

      // The newly issued key should authenticate against the public API.
      // Use chat completions (small response from the `test` provider) rather
      // than /api/v1/models — the latter ships the embedded models.dev catalog
      // and has been observed to drop the connection mid-stream on some
      // deployments under parallel-test load.
      const keyClient = apiKeyClient(issued.key);
      const chatResp = await apiV1ChatCompletions({
        client: keyClient,
        body: {
          model: "test/test-model",
          messages: [{ role: "user", content: "Hello" }],
        },
      });
      expect(chatResp.response.status).toBe(200);
    });

    it("rejects redemption with the wrong code_verifier", async () => {
      const { client } = getContext();
      const { challenge } = makePkcePair();

      const authResp = await oauthAuthorize({
        client,
        body: {
          callback_url: callbackUrl,
          code_challenge: challenge,
          code_challenge_method: "S256",
        },
      });
      expect(authResp.response.status).toBe(201);

      // Use an unrelated verifier — the SHA-256 won't match the stored challenge.
      const { verifier: wrongVerifier } = makePkcePair();
      const tokenResp = await oauthToken({
        client,
        body: {
          code: authResp.data!.code,
          code_verifier: wrongVerifier,
          code_challenge_method: "S256",
        },
      });
      expect(tokenResp.response.status).toBe(400);
    });

    it("rejects reuse of a redeemed code", async () => {
      const { client } = getContext();
      const { verifier, challenge } = makePkcePair();

      const authResp = await oauthAuthorize({
        client,
        body: {
          callback_url: callbackUrl,
          code_challenge: challenge,
          code_challenge_method: "S256",
        },
      });
      const code = authResp.data!.code;

      const first = await oauthToken({
        client,
        body: { code, code_verifier: verifier, code_challenge_method: "S256" },
      });
      expect(first.response.status).toBe(200);

      const second = await oauthToken({
        client,
        body: { code, code_verifier: verifier, code_challenge_method: "S256" },
      });
      expect(second.response.status).toBe(400);
    });

    it("rejects an http callback to a non-loopback host", async () => {
      const { client } = getContext();
      const { challenge } = makePkcePair();

      const authResp = await oauthAuthorize({
        client,
        body: {
          callback_url: "http://example.com/callback",
          code_challenge: challenge,
          code_challenge_method: "S256",
        },
      });
      expect(authResp.response.status).toBe(400);
    });

    it("rejects code_challenge_method=plain when not enabled", async () => {
      const { client } = getContext();
      // For "plain", the challenge IS the verifier — re-use a fresh verifier so
      // the length validator passes and we hit the policy check.
      const { verifier } = makePkcePair();

      const authResp = await oauthAuthorize({
        client,
        body: {
          callback_url: callbackUrl,
          code_challenge: verifier,
          code_challenge_method: "plain",
        },
      });
      expect(authResp.response.status).toBe(400);
    });
  });
}
