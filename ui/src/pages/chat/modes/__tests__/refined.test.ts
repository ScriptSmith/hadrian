import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendRefinedMode } from "../refined";
import { createMockContext } from "./test-utils";

describe("sendRefinedMode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode with single model", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const fallback = vi.fn().mockResolvedValue([{ content: "Fallback" }]);

      const results = await sendRefinedMode("Hello", ctx, fallback);

      expect(fallback).toHaveBeenCalledWith("Hello");
      expect(results).toEqual([{ content: "Fallback" }]);
    });

    it("falls back when refinementRounds is 1", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { refinementRounds: 1 },
      });
      const fallback = vi.fn().mockResolvedValue([{ content: "Fallback" }]);

      await sendRefinedMode("Hello", ctx, fallback);

      expect(fallback).toHaveBeenCalled();
    });

    it("does not call streamResponse when falling back", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const fallback = vi.fn().mockResolvedValue([]);

      await sendRefinedMode("Hello", ctx, fallback);

      expect(streamResponse).not.toHaveBeenCalled();
    });
  });

  describe("initial response phase", () => {
    it("starts with initial phase for first model", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response", usage: { inputTokens: 10 } });

      await sendRefinedMode("Hello", ctx, vi.fn());

      // First call should set initial phase
      expect(setModeState).toHaveBeenNthCalledWith(1, {
        mode: "refined",
        phase: "initial",
        currentRound: 0,
        totalRounds: 2,
        currentModel: "model-a",
        rounds: [],
      });
    });

    it("sends user message to first model", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendRefinedMode("What is 2+2?", ctx, vi.fn());

      const firstCallInputItems = streamResponse.mock.calls[0][1];
      expect(firstCallInputItems).toHaveLength(1);
      expect(firstCallInputItems[0].role).toBe("user");
      expect(firstCallInputItems[0].content).toBe("What is 2+2?");
    });

    it("returns nulls for all models when initial response fails", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce(null);

      const results = await sendRefinedMode("Hello", ctx, vi.fn());

      expect(results).toHaveLength(3);
      expect(results).toEqual([null, null, null]);
    });

    it("records initial round in refinement history", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({
          content: "Initial response",
          usage: { inputTokens: 10, outputTokens: 20 },
        })
        .mockResolvedValueOnce({ content: "Refined response" });

      await sendRefinedMode("Hello", ctx, vi.fn());

      // Runner uses setModeState for all state updates (including round additions)
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      expect(setModeState).toHaveBeenCalled();
      // Verify rounds are recorded in the state
      const finalCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(finalCall.rounds.length).toBeGreaterThan(0);
    });
  });

  describe("refinement phase", () => {
    it("transitions to refining phase for subsequent rounds", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial response" })
        .mockResolvedValueOnce({ content: "Refined response" });

      await sendRefinedMode("Hello", ctx, vi.fn());

      // Runner uses setModeState for phase transitions
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      // Find a call with phase: "refining"
      const refiningCall = setModeState.mock.calls.find(
        (call) => call[0].mode === "refined" && call[0].phase === "refining"
      );
      expect(refiningCall).toBeDefined();
    });

    it("sends refinement prompt with previous response to refining model", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial response content" })
        .mockResolvedValueOnce({ content: "Refined response" });

      await sendRefinedMode("What is 2+2?", ctx, vi.fn());

      // Second model receives system prompt with previous response
      const secondCallInputItems = streamResponse.mock.calls[1][1];
      expect(secondCallInputItems).toHaveLength(2);

      // System prompt contains the previous response
      expect(secondCallInputItems[0].role).toBe("system");
      expect(secondCallInputItems[0].content).toContain("Initial response content");
      expect(secondCallInputItems[0].content).toContain("refining");

      // User message
      expect(secondCallInputItems[1].role).toBe("user");
      expect(secondCallInputItems[1].content).toBe("What is 2+2?");
    });

    it("uses custom refinement prompt from modeConfig", async () => {
      const customPrompt = "Custom refinement: {previous_response}";
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { refinementPrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial" })
        .mockResolvedValueOnce({ content: "Refined" });

      await sendRefinedMode("Test", ctx, vi.fn());

      const secondCallInputItems = streamResponse.mock.calls[1][1];
      // Custom prompt is used (note: it doesn't replace {previous_response} when custom prompt is provided)
      expect(secondCallInputItems[0].content).toBe(customPrompt);
    });

    it("records refinement round in history", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Initial" }).mockResolvedValueOnce({
        content: "Refined",
        usage: { inputTokens: 15, outputTokens: 25 },
      });

      await sendRefinedMode("Hello", ctx, vi.fn());

      // Runner uses setModeState for all state updates
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      expect(setModeState).toHaveBeenCalled();
      // Final state should have 2 rounds (initial + refinement)
      const finalCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(finalCall.rounds).toHaveLength(2);
      expect(finalCall.rounds[1].model).toBe("model-b");
      expect(finalCall.rounds[1].content).toBe("Refined");
    });

    it("stops refinement when a round fails", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { refinementRounds: 3 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial" })
        .mockResolvedValueOnce(null) // Second round fails
        .mockResolvedValueOnce({ content: "Should not be called" });

      await sendRefinedMode("Test", ctx, vi.fn());

      // Should only call twice (initial + first refinement that fails)
      expect(streamResponse).toHaveBeenCalledTimes(2);
    });
  });

  describe("rounds configuration", () => {
    it("defaults to 2 rounds", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendRefinedMode("Test", ctx, vi.fn());

      // Default 2 rounds = 2 calls
      expect(streamResponse).toHaveBeenCalledTimes(2);
    });

    it("respects refinementRounds config", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c", "model-d"],
        modeConfig: { refinementRounds: 4 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendRefinedMode("Test", ctx, vi.fn());

      expect(streamResponse).toHaveBeenCalledTimes(4);
    });

    it("limits rounds to number of models", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { refinementRounds: 10 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendRefinedMode("Test", ctx, vi.fn());

      // Only 2 models, so max 2 rounds
      expect(streamResponse).toHaveBeenCalledTimes(2);
    });

    it("cycles through models for rounds", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { refinementRounds: 2 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const callOrder: string[] = [];

      streamResponse.mockImplementation(async (model: string) => {
        callOrder.push(model);
        return { content: "Response" };
      });

      await sendRefinedMode("Test", ctx, vi.fn());

      expect(callOrder).toEqual(["model-a", "model-b"]);
    });
  });

  describe("result construction", () => {
    it("returns only final model result with others null", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial" })
        .mockResolvedValueOnce({ content: "Final refined content" });

      const results = await sendRefinedMode("Test", ctx, vi.fn());

      expect(results).toHaveLength(3);
      expect(results[0]).toBeNull(); // model-a (initial, but not final)
      expect(results[1]?.content).toBe("Final refined content"); // model-b (final)
      expect(results[2]).toBeNull(); // model-c (not used)
    });

    it("includes refinement history in metadata", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({
          content: "Initial",
          usage: { inputTokens: 10, outputTokens: 20 },
        })
        .mockResolvedValueOnce({
          content: "Refined",
          usage: { inputTokens: 15, outputTokens: 25 },
        });

      const results = await sendRefinedMode("Test", ctx, vi.fn());

      expect(results[1]?.modeMetadata).toEqual({
        mode: "refined",
        isRefined: true,
        refinementRound: 1,
        totalRounds: 2,
        refinementHistory: [
          { model: "model-a", content: "Initial", usage: { inputTokens: 10, outputTokens: 20 } },
          { model: "model-b", content: "Refined", usage: { inputTokens: 15, outputTokens: 25 } },
        ],
      });
    });

    it("uses content from last successful round when refinement fails", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { refinementRounds: 3 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Initial" }).mockResolvedValueOnce(null); // Second round fails

      const results = await sendRefinedMode("Test", ctx, vi.fn());

      // Note: The code assigns finalModel at the START of each refinement round,
      // so even when model-b fails, finalModel is model-b (not model-a).
      // The content is preserved from the last successful round.
      expect(results[0]).toBeNull();
      expect(results[1]?.content).toBe("Initial"); // Content from model-a, attributed to model-b
      expect(results[2]).toBeNull();
    });
  });

  describe("state management", () => {
    it("initializes streaming for current instance each round", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendRefinedMode("Test", ctx, vi.fn());

      // Now called with instance IDs and a model map (backwards-compat: model ID = instance ID)
      expect(initStreaming).toHaveBeenNthCalledWith(
        1,
        ["model-a"],
        new Map([["model-a", "model-a"]])
      );
      expect(initStreaming).toHaveBeenNthCalledWith(
        2,
        ["model-b"],
        new Map([["model-b", "model-b"]])
      );
    });

    it("sets done phase at end", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial" })
        .mockResolvedValueOnce({ content: "Refined" });

      await sendRefinedMode("Test", ctx, vi.fn());

      // Last setModeState call should be done phase
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1];
      expect(lastCall[0].phase).toBe("done");
      expect(lastCall[0].rounds).toHaveLength(2); // refinement history
    });
  });

  describe("abort handling", () => {
    it("passes abort controller to streamResponse", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const receivedControllers: AbortController[] = [];

      streamResponse.mockImplementation(
        async (_model: string, _input: unknown, controller: AbortController) => {
          receivedControllers.push(controller);
          return { content: "Response" };
        }
      );

      await sendRefinedMode("Test", ctx, vi.fn());

      expect(receivedControllers).toHaveLength(2);
      expect(receivedControllers[0]).toBeInstanceOf(AbortController);
      expect(receivedControllers[1]).toBeInstanceOf(AbortController);
    });

    it("updates abortControllersRef for each round", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const refValues: AbortController[][] = [];

      streamResponse.mockImplementation(async () => {
        refValues.push([...ctx.abortControllersRef.current]);
        return { content: "Response" };
      });

      await sendRefinedMode("Test", ctx, vi.fn());

      expect(refValues).toHaveLength(2);
      expect(refValues[0]).toHaveLength(1);
      expect(refValues[1]).toHaveLength(1);
      // Controllers should be different for each round
      expect(refValues[0][0]).not.toBe(refValues[1][0]);
    });

    it("stops refinement when abort signal is triggered", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { refinementRounds: 3 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockImplementation(async () => {
        // Abort after first call by calling abort() on the controller
        if (streamResponse.mock.calls.length === 1) {
          ctx.abortControllersRef.current[0]?.abort();
        }
        return { content: "Response" };
      });

      await sendRefinedMode("Test", ctx, vi.fn());

      // Should stop after initial response (before refinement)
      expect(streamResponse).toHaveBeenCalledTimes(1);
    });
  });

  describe("message filtering", () => {
    it("filters messages for initial model", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Hello", timestamp: new Date() },
      ];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendRefinedMode("Test", ctx, vi.fn());

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-a");
    });

    it("includes conversation history in initial model input", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Previous question", timestamp: new Date() },
        {
          id: "2",
          role: "assistant" as const,
          content: "Previous answer",
          model: "model-a",
          timestamp: new Date(),
        },
      ];
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        messages,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendRefinedMode("New question", ctx, vi.fn());

      const inputItems = streamResponse.mock.calls[0][1];

      // Should include conversation history followed by new user message
      expect(inputItems).toHaveLength(3);
      expect(inputItems[0].role).toBe("user");
      expect(inputItems[0].content).toBe("Previous question");
      expect(inputItems[1].role).toBe("assistant");
      expect(inputItems[1].content).toBe("Previous answer");
      expect(inputItems[2].role).toBe("user");
      expect(inputItems[2].content).toBe("New question");
    });
  });

  describe("multimodal content", () => {
    it("handles array content (multimodal) for initial message", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const multimodalContent = [
        { type: "input_text", text: "Describe this image" },
        { type: "input_image", source: { type: "base64", data: "abc123" } },
      ];

      await sendRefinedMode(multimodalContent, ctx, vi.fn());

      const inputItems = streamResponse.mock.calls[0][1];
      expect(inputItems[0].role).toBe("user");
      expect(inputItems[0].content).toEqual(multimodalContent);
    });

    it("extracts text from multimodal content for refinement prompts", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial description" })
        .mockResolvedValueOnce({ content: "Refined description" });

      const multimodalContent = [
        { type: "input_text", text: "Describe this image" },
        { type: "input_image", source: { type: "base64", data: "abc123" } },
      ];

      await sendRefinedMode(multimodalContent, ctx, vi.fn());

      // Refinement model should receive text extraction of the original question
      const secondCallInputItems = streamResponse.mock.calls[1][1];
      expect(secondCallInputItems[1].role).toBe("user");
      expect(secondCallInputItems[1].content).toBe("Describe this image");
    });
  });
});
