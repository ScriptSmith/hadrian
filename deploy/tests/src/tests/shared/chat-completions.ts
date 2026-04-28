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
      // The generated client types this as `{}`; structurally validate the
      // OpenAI-shaped response so the test catches breakage in content/usage
      // shape, not just status code.
      const data = response.data as
        | {
            model?: string;
            choices?: Array<{
              message?: { role?: string; content?: string };
              finish_reason?: string;
            }>;
            usage?: {
              prompt_tokens?: number;
              completion_tokens?: number;
              total_tokens?: number;
            };
          }
        | undefined;
      expect(data).toBeDefined();

      // Echoes back the requested model (or a downstream alias of it).
      expect(typeof data!.model).toBe("string");
      expect(data!.model!.length).toBeGreaterThan(0);

      // Choices: at least one, with a non-empty assistant message and a
      // finish_reason. We don't pin specific text because providers vary.
      const choices = data!.choices;
      expect(Array.isArray(choices)).toBe(true);
      expect(choices!.length).toBeGreaterThan(0);
      const choice = choices![0];
      expect(choice.message).toBeDefined();
      expect(choice.message!.role).toBe("assistant");
      expect(typeof choice.message!.content).toBe("string");
      expect(choice.message!.content!.length).toBeGreaterThan(0);
      expect(typeof choice.finish_reason).toBe("string");

      // Usage block must report at least the prompt tokens; total tokens
      // should equal prompt + completion when both are present.
      expect(data!.usage).toBeDefined();
      const usage = data!.usage!;
      expect(usage.prompt_tokens!).toBeGreaterThanOrEqual(1);
      expect(usage.completion_tokens!).toBeGreaterThanOrEqual(0);
      expect(usage.total_tokens).toBe(
        usage.prompt_tokens! + usage.completion_tokens!,
      );
    });
  });
}
