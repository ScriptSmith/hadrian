import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendChainedMode } from "../chained";
import { createMockContext } from "./test-utils";

describe("sendChainedMode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("sequential execution", () => {
    it("executes models in order, one at a time", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const callOrder: string[] = [];

      streamResponse.mockImplementation(async (model: string) => {
        callOrder.push(model);
        return { content: `Response from ${model}`, usage: { inputTokens: 10, outputTokens: 20 } };
      });

      await sendChainedMode("Hello", ctx);

      // Verify sequential order
      expect(callOrder).toEqual(["model-a", "model-b", "model-c"]);
      expect(streamResponse).toHaveBeenCalledTimes(3);
    });

    it("returns results for all models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A" })
        .mockResolvedValueOnce({ content: "Response B" });

      const results = await sendChainedMode("Hello", ctx);

      expect(results).toHaveLength(2);
      expect(results[0]?.content).toBe("Response A");
      expect(results[1]?.content).toBe("Response B");
    });
  });

  describe("chain context building", () => {
    it("first model only sees user message", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A" })
        .mockResolvedValueOnce({ content: "Response B" });

      await sendChainedMode("What is 2+2?", ctx);

      // First model's input items
      const firstCallInputItems = streamResponse.mock.calls[0][1];

      // Should have only the user message (no previous chain responses)
      expect(firstCallInputItems).toHaveLength(1);
      expect(firstCallInputItems[0].role).toBe("user");
      expect(firstCallInputItems[0].content).toBe("What is 2+2?");
    });

    it("subsequent models see previous responses as assistant messages", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "First response" })
        .mockResolvedValueOnce({ content: "Second response" })
        .mockResolvedValueOnce({ content: "Third response" });

      await sendChainedMode("Question", ctx);

      // Second model should see first response
      const secondCallInputItems = streamResponse.mock.calls[1][1];
      expect(secondCallInputItems).toHaveLength(3); // user + assistant + build-upon instruction

      // User message
      expect(secondCallInputItems[0].role).toBe("user");
      expect(secondCallInputItems[0].content).toBe("Question");

      // First model's response as assistant
      expect(secondCallInputItems[1].role).toBe("assistant");
      expect(secondCallInputItems[1].content).toContain("[model-a]:");
      expect(secondCallInputItems[1].content).toContain("First response");

      // Build-upon instruction
      expect(secondCallInputItems[2].role).toBe("user");
      expect(secondCallInputItems[2].content).toContain("build upon");

      // Third model should see both previous responses
      const thirdCallInputItems = streamResponse.mock.calls[2][1];
      expect(thirdCallInputItems).toHaveLength(4); // user + 2 assistants + build-upon

      expect(thirdCallInputItems[1].content).toContain("[model-a]:");
      expect(thirdCallInputItems[1].content).toContain("First response");
      expect(thirdCallInputItems[2].content).toContain("[model-b]:");
      expect(thirdCallInputItems[2].content).toContain("Second response");
    });

    it("includes build-upon instruction for non-first models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A" })
        .mockResolvedValueOnce({ content: "Response B" });

      await sendChainedMode("Test", ctx);

      // First model should NOT have build-upon instruction
      const firstCallInputItems = streamResponse.mock.calls[0][1];
      const firstHasBuildUpon = firstCallInputItems.some(
        (item: { content: string }) =>
          typeof item.content === "string" && item.content.includes("build upon")
      );
      expect(firstHasBuildUpon).toBe(false);

      // Second model SHOULD have build-upon instruction
      const secondCallInputItems = streamResponse.mock.calls[1][1];
      const lastItem = secondCallInputItems[secondCallInputItems.length - 1];
      expect(lastItem.role).toBe("user");
      expect(lastItem.content).toContain("build upon");
      expect(lastItem.content).toContain("refine");
      expect(lastItem.content).toContain("improve");
    });
  });

  describe("chain position metadata", () => {
    it("includes correct chain position in each result", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A" })
        .mockResolvedValueOnce({ content: "B" })
        .mockResolvedValueOnce({ content: "C" });

      const results = await sendChainedMode("Test", ctx);

      expect(results[0]?.modeMetadata).toEqual({
        mode: "chained",
        chainPosition: 0,
        chainTotal: 3,
      });
      expect(results[1]?.modeMetadata).toEqual({
        mode: "chained",
        chainPosition: 1,
        chainTotal: 3,
      });
      expect(results[2]?.modeMetadata).toEqual({
        mode: "chained",
        chainPosition: 2,
        chainTotal: 3,
      });
    });
  });

  describe("state management", () => {
    it("initializes streaming for all instances", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendChainedMode("Test", ctx);

      // Now called with instance IDs and a model map (backwards-compat: model ID = instance ID)
      const expectedModelMap = new Map([
        ["model-a", "model-a"],
        ["model-b", "model-b"],
      ]);
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(
        ["model-a", "model-b"],
        expectedModelMap
      );
    });

    it("updates chain position before each model", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendChainedMode("Test", ctx);

      // Initial position (called before loop AND at start of first iteration)
      expect(setModeState).toHaveBeenNthCalledWith(1, { mode: "chained", position: [0, 3] });
      expect(setModeState).toHaveBeenNthCalledWith(2, { mode: "chained", position: [0, 3] });
      expect(setModeState).toHaveBeenNthCalledWith(3, { mode: "chained", position: [1, 3] });
      expect(setModeState).toHaveBeenNthCalledWith(4, { mode: "chained", position: [2, 3] });
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

      await sendChainedMode("Test", ctx);

      // Each model should receive an AbortController
      expect(receivedControllers).toHaveLength(2);
      expect(receivedControllers[0]).toBeInstanceOf(AbortController);
      expect(receivedControllers[1]).toBeInstanceOf(AbortController);
    });

    it("updates abortControllersRef with current controller", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const refValues: AbortController[][] = [];

      streamResponse.mockImplementation(async () => {
        // Capture the current state of abortControllersRef
        refValues.push([...ctx.abortControllersRef.current]);
        return { content: "Response" };
      });

      await sendChainedMode("Test", ctx);

      // Each call should have had the ref updated with the current controller
      expect(refValues).toHaveLength(2);
      expect(refValues[0]).toHaveLength(1);
      expect(refValues[1]).toHaveLength(1);
      // Controllers should be different for each model
      expect(refValues[0][0]).not.toBe(refValues[1][0]);
    });
  });

  describe("message filtering", () => {
    it("filters messages for each model using filterMessagesForModel", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Hello", timestamp: new Date() },
        {
          id: "2",
          role: "assistant" as const,
          content: "Hi",
          model: "model-a",
          timestamp: new Date(),
        },
      ];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendChainedMode("Test", ctx);

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-a");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-b");
    });

    it("includes filtered conversation history in input items", async () => {
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
        models: ["model-a"],
        messages,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "New response" });

      await sendChainedMode("New question", ctx);

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

  describe("abort controller management", () => {
    it("creates new abort controller for each model", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const controllers: AbortController[] = [];

      streamResponse.mockImplementation(
        async (_model: string, _input: unknown, controller: AbortController) => {
          controllers.push(controller);
          return { content: "Response" };
        }
      );

      await sendChainedMode("Test", ctx);

      expect(controllers).toHaveLength(3);
      // Each controller should be a unique instance
      expect(controllers[0]).not.toBe(controllers[1]);
      expect(controllers[1]).not.toBe(controllers[2]);
      expect(controllers[0]).toBeInstanceOf(AbortController);
    });
  });

  describe("failure handling", () => {
    it("returns null for failed model and continues chain", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A" })
        .mockResolvedValueOnce(null) // model-b fails
        .mockResolvedValueOnce({ content: "Response C" });

      const results = await sendChainedMode("Test", ctx);

      expect(results[0]?.content).toBe("Response A");
      expect(results[1]).toBeNull();
      expect(results[2]?.content).toBe("Response C");
    });

    it("does not include failed responses in chain context", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A" })
        .mockResolvedValueOnce(null) // model-b fails
        .mockResolvedValueOnce({ content: "Response C" });

      await sendChainedMode("Test", ctx);

      // Third model should only see first model's response (not the failed one)
      const thirdCallInputItems = streamResponse.mock.calls[2][1];

      // Should have: user message + 1 assistant (model-a) + build-upon instruction
      expect(thirdCallInputItems).toHaveLength(3);

      // The assistant message should only be from model-a
      const assistantMessages = thirdCallInputItems.filter(
        (item: { role: string }) => item.role === "assistant"
      );
      expect(assistantMessages).toHaveLength(1);
      expect(assistantMessages[0].content).toContain("[model-a]:");
    });
  });

  describe("multimodal content", () => {
    it("handles array content (multimodal)", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const multimodalContent = [
        { type: "input_text", text: "Describe this image" },
        { type: "input_image", source: { type: "base64", data: "abc123" } },
      ];

      await sendChainedMode(multimodalContent, ctx);

      const inputItems = streamResponse.mock.calls[0][1];
      expect(inputItems[0].role).toBe("user");
      expect(inputItems[0].content).toEqual(multimodalContent);
    });
  });

  describe("single model", () => {
    it("works with a single model (no chaining needed)", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Only response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      const results = await sendChainedMode("Test", ctx);

      expect(results).toHaveLength(1);
      expect(results[0]?.content).toBe("Only response");
      expect(results[0]?.modeMetadata).toEqual({
        mode: "chained",
        chainPosition: 0,
        chainTotal: 1,
      });
    });
  });
});
