/**
 * Chat completions API tests (Test 11 from bash)
 *
 * Tests the chat completions API with API key authentication.
 */
import { describe, it, expect, beforeAll } from "vitest";
import type { Client } from "../../client/client";
import {
  organizationCreate,
  apiKeyCreate,
  apiV1ChatCompletions,
} from "../../client";

export interface ChatCompletionsContext {
  url: string;
  client: Client;
  apiKeyClient: (apiKey: string) => Client;
  testName: string;
}

/**
 * Run chat completions API tests.
 * @param getContext - Function that returns the test context. Called lazily to ensure
 *                     the context is available after beforeAll setup completes.
 */
export function runChatCompletionsTests(
  getContext: () => ChatCompletionsContext
) {
  describe("Chat Completions API", () => {
    let apiKey: string;

    beforeAll(async () => {
      const { client, testName } = getContext();

      // Create an organization for chat tests
      const orgResponse = await organizationCreate({
        client,
        body: {
          slug: `${testName}-chat-org`,
          name: `Chat Test Organization for ${testName}`,
        },
      });
      const orgId = orgResponse.data!.id;

      // Create an API key for the chat completions test
      const keyResponse = await apiKeyCreate({
        client,
        body: {
          name: "Chat Test Key",
          owner: {
            type: "organization",
            org_id: orgId,
          },
        },
      });
      apiKey = keyResponse.data!.key!;
    });

    // Test 11: Chat completions API with API key
    it("can send a chat completion request with API key authentication", async () => {
      const { apiKeyClient } = getContext();

      // Create a client with the API key for authentication
      const chatClient = apiKeyClient(apiKey);

      const response = await apiV1ChatCompletions({
        client: chatClient,
        body: {
          model: "test/test-model",
          messages: [{ role: "user", content: "Hello" }],
        },
      });

      expect(response.response.status).toBe(200);
      expect(response.data).toBeDefined();
      // The response should have choices array
      // Note: The actual response structure depends on the server
      // For the test provider, it returns a mock response
    });
  });
}
