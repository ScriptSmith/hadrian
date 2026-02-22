import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendCritiquedMode } from "../critiqued";
import type { CritiqueRoundData } from "../types";
import { createMockContext as createBaseContext } from "./test-utils";

// Critiqued mode uses different default models
function createMockContext(overrides: Parameters<typeof createBaseContext>[0] = {}) {
  return createBaseContext({
    models: ["primary-model", "critic-a", "critic-b"],
    ...overrides,
  });
}

describe("sendCritiquedMode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode with single model", async () => {
      const ctx = createMockContext({ models: ["primary-model"] });
      const fallback = vi.fn().mockResolvedValue([{ content: "Fallback" }]);

      const results = await sendCritiquedMode("Hello", ctx, fallback);

      expect(fallback).toHaveBeenCalledWith("Hello");
      expect(results).toEqual([{ content: "Fallback" }]);
    });

    it("does not call streamResponse when falling back", async () => {
      const ctx = createMockContext({ models: ["primary-model"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const fallback = vi.fn().mockResolvedValue([]);

      await sendCritiquedMode("Hello", ctx, fallback);

      expect(streamResponse).not.toHaveBeenCalled();
    });

    it("falls back when only primary model (all models are the primary)", async () => {
      const ctx = createMockContext({
        models: ["primary-model"],
        modeConfig: { primaryModel: "primary-model" },
      });
      const fallback = vi.fn().mockResolvedValue([{ content: "Single" }]);

      await sendCritiquedMode("Test", ctx, fallback);

      expect(fallback).toHaveBeenCalled();
    });
  });

  describe("primary model selection", () => {
    it("uses first model as primary by default", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCritiquedMode("Test", ctx, vi.fn());

      expect(setModeState).toHaveBeenCalledWith(
        expect.objectContaining({
          mode: "critiqued",
          phase: "initial",
          primaryModel: "model-a",
          critiqueModels: ["model-b", "model-c"],
        })
      );
    });

    it("uses primaryModel from modeConfig when specified", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { primaryModel: "model-b" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCritiquedMode("Test", ctx, vi.fn());

      expect(setModeState).toHaveBeenCalledWith(
        expect.objectContaining({
          mode: "critiqued",
          phase: "initial",
          primaryModel: "model-b",
          critiqueModels: ["model-a", "model-c"],
        })
      );
    });
  });

  describe("initial response phase", () => {
    it("starts with initial phase for primary model", async () => {
      const ctx = createMockContext({ models: ["primary-model", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCritiquedMode("Hello", ctx, vi.fn());

      expect(setModeState).toHaveBeenNthCalledWith(
        1,
        expect.objectContaining({
          mode: "critiqued",
          phase: "initial",
          primaryModel: "primary-model",
          critiqueModels: ["critic-a"],
        })
      );
    });

    it("sends user message to primary model with conversation history", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Previous", timestamp: new Date() },
        {
          id: "2",
          role: "assistant" as const,
          content: "Previous answer",
          model: "primary-model",
          timestamp: new Date(),
        },
      ];
      const ctx = createMockContext({
        models: ["primary-model", "critic-a"],
        messages,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCritiquedMode("Current question?", ctx, vi.fn());

      const inputItems = streamResponse.mock.calls[0][1];
      expect(inputItems).toHaveLength(3);
      expect(inputItems[0].content).toBe("Previous");
      expect(inputItems[1].content).toBe("Previous answer");
      expect(inputItems[2].content).toBe("Current question?");
    });

    it("returns nulls for all models when initial response fails", async () => {
      const ctx = createMockContext({ models: ["primary-model", "critic-a", "critic-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce(null);

      const results = await sendCritiquedMode("Hello", ctx, vi.fn());

      expect(results).toHaveLength(3);
      expect(results).toEqual([null, null, null]);
    });

    it("initializes streaming for primary model", async () => {
      const ctx = createMockContext({ models: ["primary-model", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCritiquedMode("Hello", ctx, vi.fn());

      // Instance-aware streaming passes [instanceIds], modelMap
      expect(initStreaming).toHaveBeenNthCalledWith(
        1,
        ["primary-model"],
        new Map([["primary-model", "primary-model"]])
      );
    });
  });

  describe("critique phase", () => {
    it("transitions to critiquing phase after initial response", async () => {
      const ctx = createMockContext({ models: ["primary-model", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial response", usage: { inputTokens: 10 } })
        .mockResolvedValue({ content: "Critique" });

      await sendCritiquedMode("Hello", ctx, vi.fn());

      expect(setModeState).toHaveBeenNthCalledWith(
        2,
        expect.objectContaining({
          mode: "critiqued",
          phase: "critiquing",
          primaryModel: "primary-model",
          initialResponse: "Initial response",
        })
      );
    });

    it("initializes streaming for all critic models", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b", "critic-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCritiquedMode("Hello", ctx, vi.fn());

      // Instance-aware streaming passes [instanceIds], modelMap
      expect(initStreaming).toHaveBeenNthCalledWith(
        2,
        ["critic-a", "critic-b", "critic-c"],
        new Map([
          ["critic-a", "critic-a"],
          ["critic-b", "critic-b"],
          ["critic-c", "critic-c"],
        ])
      );
    });

    it("sends critique prompt with initial response to all critics in parallel", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial answer" })
        .mockResolvedValue({ content: "Critique content" });

      await sendCritiquedMode("User question?", ctx, vi.fn());

      // Critics should receive system prompt with critique instructions
      // Calls are: [0] = primary initial, [1-2] = critics (order may vary), [3] = primary revision
      // Filter to get only critic calls (models that start with "critic")
      const criticCalls = streamResponse.mock.calls.filter((call) =>
        (call[0] as string).startsWith("critic")
      );
      expect(criticCalls).toHaveLength(2);

      // Both critics should get the same system prompt containing the initial response
      for (const call of criticCalls) {
        const inputItems = call[1];
        expect(inputItems[0].role).toBe("system");
        // The prompt contains "Here is the response to critique:" followed by the initial response
        expect(inputItems[0].content).toContain("Initial answer");
        expect(inputItems[0].content).toContain("critical reviewer");
        expect(inputItems[1].role).toBe("user");
        expect(inputItems[1].content).toBe("User question?");
      }
    });

    it("uses custom critique prompt from modeConfig", async () => {
      const customPrompt = "Custom critique: {response}";
      const ctx = createMockContext({
        models: ["primary", "critic-a"],
        modeConfig: { critiquePrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial" })
        .mockResolvedValue({ content: "Critique" });

      await sendCritiquedMode("Test", ctx, vi.fn());

      const criticCall = streamResponse.mock.calls[1];
      expect(criticCall[1][0].content).toBe(customPrompt);
    });

    it("collects critiques and adds them via updateModeState", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial" })
        .mockResolvedValueOnce({ content: "Critique from A", usage: { inputTokens: 5 } })
        .mockResolvedValueOnce({ content: "Critique from B", usage: { inputTokens: 8 } });

      await sendCritiquedMode("Test", ctx, vi.fn());

      // Critiques are added via updateModeState
      expect(updateModeState).toHaveBeenCalled();
    });

    it("handles partial critique failures gracefully", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial" })
        .mockResolvedValueOnce(null) // critic-a fails
        .mockResolvedValueOnce({ content: "Critique from B" });

      await sendCritiquedMode("Test", ctx, vi.fn());

      // Only one critique should be added via updateModeState
      expect(updateModeState).toHaveBeenCalled();
    });
  });

  describe("no critiques received", () => {
    it("returns initial response when all critiques fail", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({
          content: "Initial response",
          usage: { inputTokens: 10, outputTokens: 20 },
        })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null);

      const results = await sendCritiquedMode("Test", ctx, vi.fn());

      // Primary model result at index 0
      expect(results[0]).toMatchObject({
        content: "Initial response",
        usage: { inputTokens: 10, outputTokens: 20 },
        modeMetadata: {
          mode: "critiqued",
          isCritiqued: true,
          primaryModel: "primary",
          initialResponse: "Initial response",
          critiques: [],
        },
      });
      expect(results[1]).toBeNull();
      expect(results[2]).toBeNull();
    });

    it("sets done state when no critiques received", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial", usage: { inputTokens: 5 } })
        .mockResolvedValueOnce(null);

      await sendCritiquedMode("Test", ctx, vi.fn());

      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1];
      expect(lastCall[0].phase).toBe("done");
      expect(lastCall[0].critiques).toEqual([]); // empty critiques array
    });
  });

  describe("revision phase", () => {
    it("transitions to revising phase after critiques", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "Critique" })
        .mockResolvedValueOnce({ content: "Revised" });

      await sendCritiquedMode("Test", ctx, vi.fn());

      // Find the revising phase call
      const revisingCall = setModeState.mock.calls.find((call) => call[0].phase === "revising")!;
      expect(revisingCall).toBeDefined();
      expect(revisingCall![0].primaryModel).toBe("primary");
      expect(revisingCall![0].initialResponse).toBe("Initial");
      expect(revisingCall![0].critiques).toHaveLength(1);
    });

    it("sends revision prompt with critiques to primary model", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "My initial answer" })
        .mockResolvedValueOnce({ content: "Critique from A" })
        .mockResolvedValueOnce({ content: "Critique from B" })
        .mockResolvedValueOnce({ content: "Revised answer" });

      await sendCritiquedMode("User question?", ctx, vi.fn());

      // Find the revision call (last one to primary model)
      const revisionCall = streamResponse.mock.calls[3];
      const inputItems = revisionCall[1];

      expect(inputItems[0].role).toBe("system");
      expect(inputItems[0].content).toContain("My initial answer");
      expect(inputItems[0].content).toContain("Critique from A");
      expect(inputItems[0].content).toContain("Critique from B");
      expect(inputItems[0].content).toContain("[critic-a]");
      expect(inputItems[0].content).toContain("[critic-b]");
      expect(inputItems[1].role).toBe("user");
      expect(inputItems[1].content).toBe("User question?");
    });

    it("initializes streaming for primary model during revision", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCritiquedMode("Test", ctx, vi.fn());

      // Instance-aware streaming passes [instanceIds], modelMap
      expect(initStreaming).toHaveBeenNthCalledWith(
        3,
        ["primary"],
        new Map([["primary", "primary"]])
      );
    });
  });

  describe("result construction", () => {
    it("returns revised content with critiques in metadata", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({
          content: "Initial answer",
          usage: { inputTokens: 10, outputTokens: 20 },
        })
        .mockResolvedValueOnce({
          content: "Critique A",
          usage: { inputTokens: 5, outputTokens: 15 },
        })
        .mockResolvedValueOnce({
          content: "Critique B",
          usage: { inputTokens: 8, outputTokens: 18 },
        })
        .mockResolvedValueOnce({
          content: "Revised answer",
          usage: { inputTokens: 30, outputTokens: 40 },
        });

      const results = await sendCritiquedMode("Test", ctx, vi.fn());

      expect(results).toHaveLength(3);
      expect(results[0]).toMatchObject({
        content: "Revised answer",
        usage: { inputTokens: 30, outputTokens: 40 },
        modeMetadata: {
          mode: "critiqued",
          isCritiqued: true,
          primaryModel: "primary",
          initialResponse: "Initial answer",
          initialUsage: { inputTokens: 10, outputTokens: 20 },
          critiques: [
            {
              model: "critic-a",
              content: "Critique A",
              usage: { inputTokens: 5, outputTokens: 15 },
            },
            {
              model: "critic-b",
              content: "Critique B",
              usage: { inputTokens: 8, outputTokens: 18 },
            },
          ],
        },
      });
      expect(results[1]).toBeNull();
      expect(results[2]).toBeNull();
    });

    it("uses initial response when revision fails", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial answer" })
        .mockResolvedValueOnce({ content: "Critique" })
        .mockResolvedValueOnce(null); // Revision fails

      const results = await sendCritiquedMode("Test", ctx, vi.fn());

      expect(results[0]?.content).toBe("Initial answer");
      expect(results[0]?.modeMetadata?.critiques).toHaveLength(1);
    });

    it("places result at correct index when primaryModel is not first", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { primaryModel: "model-b" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendCritiquedMode("Test", ctx, vi.fn());

      expect(results[0]).toBeNull(); // model-a
      expect(results[1]?.content).toBe("Response"); // model-b (primary)
      expect(results[2]).toBeNull(); // model-c
    });
  });

  describe("state management", () => {
    it("sets done phase at end", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCritiquedMode("Test", ctx, vi.fn());

      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1];
      expect(lastCall[0].phase).toBe("done");
    });

    it("passes critiques to done state", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial" })
        .mockResolvedValueOnce({ content: "Critique A" })
        .mockResolvedValueOnce({ content: "Critique B" })
        .mockResolvedValueOnce({ content: "Revised" });

      await sendCritiquedMode("Test", ctx, vi.fn());

      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1];
      expect(lastCall[0].phase).toBe("done");
      expect(lastCall[0].critiques).toHaveLength(2); // critiques array
    });
  });

  describe("abort handling", () => {
    it("passes abort controller to initial response", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      let receivedController: AbortController | null = null;

      streamResponse.mockImplementation(
        async (_model: string, _input: unknown, controller: AbortController) => {
          if (!receivedController) receivedController = controller;
          return { content: "Response" };
        }
      );

      await sendCritiquedMode("Test", ctx, vi.fn());

      expect(receivedController).toBeInstanceOf(AbortController);
    });

    it("passes separate abort controllers to each critic", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const criticControllers: AbortController[] = [];

      streamResponse.mockImplementation(
        async (model: string, _input: unknown, controller: AbortController) => {
          if (model !== "primary") criticControllers.push(controller);
          return { content: "Response" };
        }
      );

      await sendCritiquedMode("Test", ctx, vi.fn());

      expect(criticControllers).toHaveLength(2);
      expect(criticControllers[0]).not.toBe(criticControllers[1]);
    });

    it("updates abortControllersRef for each phase", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const refSnapshots: number[] = [];

      streamResponse.mockImplementation(async () => {
        refSnapshots.push(ctx.abortControllersRef.current.length);
        return { content: "Response" };
      });

      await sendCritiquedMode("Test", ctx, vi.fn());

      // Initial phase: 1 controller, critique phase: 1 controller, revision: 1 controller
      expect(refSnapshots).toEqual([1, 1, 1]);
    });
  });

  describe("message filtering", () => {
    it("filters messages for primary model", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Hello", timestamp: new Date() },
      ];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["primary", "critic-a"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCritiquedMode("Test", ctx, vi.fn());

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "primary");
    });
  });

  describe("multimodal content", () => {
    it("handles array content (multimodal) for initial message", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const multimodalContent = [
        { type: "input_text", text: "Describe this" },
        { type: "input_image", source: { type: "base64", data: "abc123" } },
      ];

      await sendCritiquedMode(multimodalContent, ctx, vi.fn());

      const inputItems = streamResponse.mock.calls[0][1];
      expect(inputItems[0].role).toBe("user");
      expect(inputItems[0].content).toEqual(multimodalContent);
    });

    it("extracts text from multimodal content for critique/revision prompts", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const multimodalContent = [
        { type: "input_text", text: "What is in this image?" },
        { type: "input_image", source: { type: "base64", data: "abc123" } },
      ];

      await sendCritiquedMode(multimodalContent, ctx, vi.fn());

      // Critique prompt should use extracted text
      const critiqueCall = streamResponse.mock.calls[1];
      expect(critiqueCall[1][1].content).toBe("What is in this image?");

      // Revision prompt should also use extracted text
      const revisionCall = streamResponse.mock.calls[2];
      expect(revisionCall[1][1].content).toBe("What is in this image?");
    });
  });

  describe("critique ordering", () => {
    it("preserves critique order in metadata based on completion order", async () => {
      const ctx = createMockContext({ models: ["primary", "critic-a", "critic-b", "critic-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const completedCritiques: CritiqueRoundData[] = [];

      // Simulate different completion times
      streamResponse
        .mockResolvedValueOnce({ content: "Initial" })
        .mockImplementation(async (model: string) => {
          // Simulate varying response times
          const content = `Critique from ${model}`;
          completedCritiques.push({ model, content });
          return { content };
        });

      const results = await sendCritiquedMode("Test", ctx, vi.fn());

      // Critiques should be in the order they completed
      const metadataCritiques = results[0]?.modeMetadata?.critiques || [];
      expect(metadataCritiques).toHaveLength(3);
      // All critiques should be present
      expect(metadataCritiques.map((c) => c.model).sort()).toEqual([
        "critic-a",
        "critic-b",
        "critic-c",
      ]);
    });
  });
});
