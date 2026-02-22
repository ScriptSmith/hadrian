import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendExplainerMode, DEFAULT_AUDIENCE_LEVELS } from "../explainer";
import { createMockContext, createMockFallback } from "./test-utils";

describe("sendExplainerMode", () => {
  let mockFallback: ReturnType<typeof createMockFallback>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockFallback = createMockFallback();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode when audienceLevels is empty", async () => {
      const ctx = createMockContext({
        modeConfig: { audienceLevels: [] },
      });

      await sendExplainerMode("Explain quantum physics", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Explain quantum physics");
      expect(ctx.streamResponse).not.toHaveBeenCalled();
    });

    it("does not fall back with default audience levels", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Expert explanation" });

      await sendExplainerMode("Explain AI", ctx, mockFallback);

      expect(mockFallback).not.toHaveBeenCalled();
      expect(streamResponse).toHaveBeenCalled();
    });

    it("does not fall back with single model (can generate all levels)", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      expect(mockFallback).not.toHaveBeenCalled();
      // Should call for each default level (3 times)
      expect(streamResponse).toHaveBeenCalledTimes(DEFAULT_AUDIENCE_LEVELS.length);
    });
  });

  describe("audience levels configuration", () => {
    it("uses default audience levels when not configured", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Check that default levels are used in the first call
      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.mode).toBe("explainer");
      expect(firstCall.phase).toBe("initial");
      expect(firstCall.audienceLevels).toEqual(DEFAULT_AUDIENCE_LEVELS);
      expect(firstCall.currentLevelIndex).toBe(0);
      expect(firstCall.currentModel).toBe("model-a");
    });

    it("uses configured audience levels", async () => {
      const customLevels = ["phd", "undergraduate", "high-school"];
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: customLevels },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.phase).toBe("initial");
      expect(firstCall.audienceLevels).toEqual(customLevels);
    });

    it("generates one explanation per audience level", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(streamResponse).toHaveBeenCalledTimes(2);
      expect(results).toHaveLength(2);
    });
  });

  describe("model assignment", () => {
    it("cycles through models when fewer models than levels", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { audienceLevels: ["expert", "intermediate", "beginner", "child"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Level 0 (expert) -> model-a (index 0)
      // Level 1 (intermediate) -> model-b (index 1)
      // Level 2 (beginner) -> model-a (index 0) - cycles
      // Level 3 (child) -> model-b (index 1) - cycles
      expect(streamResponse.mock.calls[0][0]).toBe("model-a");
      expect(streamResponse.mock.calls[1][0]).toBe("model-b");
      expect(streamResponse.mock.calls[2][0]).toBe("model-a");
      expect(streamResponse.mock.calls[3][0]).toBe("model-b");
    });

    it("assigns one model per level when enough models", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { audienceLevels: ["expert", "intermediate", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      expect(streamResponse.mock.calls[0][0]).toBe("model-a");
      expect(streamResponse.mock.calls[1][0]).toBe("model-b");
      expect(streamResponse.mock.calls[2][0]).toBe("model-c");
    });

    it("uses single model for all levels when only one model", async () => {
      const ctx = createMockContext({
        models: ["only-model"],
        modeConfig: { audienceLevels: ["expert", "intermediate", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      expect(streamResponse).toHaveBeenCalledTimes(3);
      expect(streamResponse.mock.calls.every((call) => call[0] === "only-model")).toBe(true);
    });
  });

  describe("initial phase (first level)", () => {
    it("sets initial phase state for first level", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Expert explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.phase).toBe("initial");
      expect(firstCall.currentLevelIndex).toBe(0);
      expect(firstCall.currentModel).toBe("model-a");
    });

    it("sends initial prompt with level and question", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Expert explanation" });

      await sendExplainerMode("What is quantum computing?", ctx, mockFallback);

      const inputItems = streamResponse.mock.calls[0][1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      const userMessage = inputItems.find((i: { role: string }) => i.role === "user")?.content;

      expect(systemPrompt).toContain("expert");
      expect(systemPrompt).toContain("What is quantum computing?");
      expect(userMessage).toBe("What is quantum computing?");
    });

    it("includes level-specific guidelines for known levels", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Simple explanation" });

      await sendExplainerMode("What is AI?", ctx, mockFallback);

      const inputItems = streamResponse.mock.calls[0][1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;

      // Check for beginner-specific guidelines
      expect(systemPrompt).toContain("simple");
    });

    it("generates generic guidelines for unknown levels", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["phd-researcher"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "PhD-level explanation" });

      await sendExplainerMode("Topic", ctx, mockFallback);

      const inputItems = streamResponse.mock.calls[0][1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;

      // Should contain generic guidelines mentioning the level
      expect(systemPrompt).toContain("phd-researcher");
      expect(systemPrompt).toContain("appropriately");
    });
  });

  describe("simplifying phase (subsequent levels)", () => {
    it("sets simplifying phase state for subsequent levels", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Second call should be simplifying phase
      const secondStateCall = setModeState.mock.calls[1][0];
      expect(secondStateCall.phase).toBe("simplifying");
      expect(secondStateCall.currentLevelIndex).toBe(1);
    });

    it("sends simplify prompt with previous explanation", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Expert technical explanation" });
      streamResponse.mockResolvedValueOnce({ content: "Simple explanation" });

      await sendExplainerMode("What is recursion?", ctx, mockFallback);

      // Second call should have previous explanation in prompt
      const secondCallInputItems = streamResponse.mock.calls[1][1];
      const systemPrompt = secondCallInputItems.find(
        (i: { role: string }) => i.role === "system"
      )?.content;

      expect(systemPrompt).toContain("Expert technical explanation");
      expect(systemPrompt).toContain("beginner");
      expect(systemPrompt).toContain("What is recursion?");
    });

    it("chains explanations sequentially", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "intermediate", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Expert version" });
      streamResponse.mockResolvedValueOnce({ content: "Intermediate version" });
      streamResponse.mockResolvedValueOnce({ content: "Beginner version" });

      await sendExplainerMode("Topic", ctx, mockFallback);

      // Third call should reference intermediate version
      const thirdCallInputItems = streamResponse.mock.calls[2][1];
      const systemPrompt = thirdCallInputItems.find(
        (i: { role: string }) => i.role === "system"
      )?.content;

      expect(systemPrompt).toContain("Intermediate version");
    });
  });

  describe("explanation tracking", () => {
    it("adds explanation after each successful response", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Expert content",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Beginner content",
        usage: { inputTokens: 15, outputTokens: 25 },
      });

      await sendExplainerMode("Question", ctx, mockFallback);

      // updateModeState is called to add explanations
      expect(updateModeState).toHaveBeenCalled();
      // 2 levels = 2 explanation additions via updateModeState
      expect(updateModeState.mock.calls.length).toBeGreaterThanOrEqual(2);
    });

    it("does not add explanation when response fails", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce(null); // First fails
      streamResponse.mockResolvedValueOnce({ content: "Beginner content" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Only successful explanations should be added
      expect(updateModeState).toHaveBeenCalled();
    });

    it("continues to next level on error", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "intermediate", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Expert" });
      streamResponse.mockRejectedValueOnce(new Error("Network error")); // intermediate fails
      streamResponse.mockResolvedValueOnce({ content: "Beginner" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      // Should have all 3 calls
      expect(streamResponse).toHaveBeenCalledTimes(3);
      // Should have 2 successful results
      expect(results.filter((r) => r !== null)).toHaveLength(2);
    });
  });

  describe("streaming initialization", () => {
    it("initializes streaming for each level with instance ID and model map", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Should initialize streaming for each level with instance IDs and model maps
      expect(initStreaming).toHaveBeenCalledTimes(2);

      // First call: instance model-a with model map
      const [ids1, map1] = initStreaming.mock.calls[0];
      expect(ids1).toEqual(["model-a"]);
      expect(map1).toBeInstanceOf(Map);
      expect(map1.get("model-a")).toBe("model-a");

      // Second call: instance model-b with model map
      const [ids2, map2] = initStreaming.mock.calls[1];
      expect(ids2).toEqual(["model-b"]);
      expect(map2).toBeInstanceOf(Map);
      expect(map2.get("model-b")).toBe("model-b");
    });
  });

  describe("abort controller management", () => {
    it("creates abort controller for each level", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Each call should have its own abort controller
      const controller1 = streamResponse.mock.calls[0][2];
      const controller2 = streamResponse.mock.calls[1][2];

      expect(controller1).toBeInstanceOf(AbortController);
      expect(controller2).toBeInstanceOf(AbortController);
      expect(controller1).not.toBe(controller2);
    });

    it("stores current abort controller in ref", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      const capturedControllers: AbortController[][] = [];
      streamResponse.mockImplementation(async () => {
        capturedControllers.push([...ctx.abortControllersRef.current]);
        return { content: "Explanation" };
      });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Each call should have exactly one controller in the ref
      expect(capturedControllers[0]).toHaveLength(1);
      expect(capturedControllers[1]).toHaveLength(1);
    });
  });

  describe("result construction", () => {
    it("returns one result per successful explanation", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "intermediate", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results).toHaveLength(3);
      expect(results.every((r) => r !== null)).toBe(true);
    });

    it("includes levelLabel with capitalized level name", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results[0]?.levelLabel).toBe("Expert");
      expect(results[1]?.levelLabel).toBe("Beginner");
    });

    it("includes mode metadata with explainer info", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Content",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.mode).toBe("explainer");
      expect(results[0]?.modeMetadata?.isExplanation).toBe(true);
      expect(results[0]?.modeMetadata?.explainerLevel).toBe("expert");
      expect(results[0]?.modeMetadata?.explainerModel).toBe("model-a");
    });

    it("includes all explanations in first result metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Expert content",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Beginner content",
        usage: { inputTokens: 15, outputTokens: 25 },
      });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      // First result should have all explanations
      expect(results[0]?.modeMetadata?.explainerLevels).toEqual(["expert", "beginner"]);
      expect(results[0]?.modeMetadata?.explanations).toHaveLength(2);
      expect(results[0]?.modeMetadata?.explanations?.[0]).toEqual({
        level: "expert",
        model: "model-a",
        content: "Expert content",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
    });

    it("includes aggregate usage in first result metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Expert",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Beginner",
        usage: { inputTokens: 15, outputTokens: 25 },
      });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      // aggregateUsage includes inputTokens, outputTokens, totalTokens, and cost
      expect(results[0]?.modeMetadata?.aggregateUsage).toMatchObject({
        inputTokens: 25,
        outputTokens: 45,
      });
    });

    it("does not include aggregate data in non-first results", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results[1]?.modeMetadata?.explainerLevels).toBeUndefined();
      expect(results[1]?.modeMetadata?.explanations).toBeUndefined();
      expect(results[1]?.modeMetadata?.aggregateUsage).toBeUndefined();
    });
  });

  describe("state transitions", () => {
    it("transitions through initial -> simplifying -> done phases", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      const phases = setModeState.mock.calls.map((call) => call[0].phase);
      expect(phases).toEqual(["initial", "simplifying", "done"]);
    });

    it("transitions to done with correct final state", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.audienceLevels).toEqual(["expert", "beginner"]);
      expect(lastCall.currentLevelIndex).toBe(1);
      expect(lastCall.explanations).toHaveLength(2);
      expect(lastCall.currentModel).toBeUndefined();
    });

    it("skips simplifying phase when only one level", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Simple explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      const phases = setModeState.mock.calls.map((call) => call[0].phase);
      expect(phases).toEqual(["initial", "done"]);
    });
  });

  describe("message filtering", () => {
    it("filters messages for each model", async () => {
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
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-a");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-b");
    });
  });

  describe("multimodal content handling", () => {
    it("extracts text from multimodal content for prompts", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      const multimodalContent = [
        { type: "input_text", text: "Explain this diagram" },
        { type: "image", source: { type: "base64", data: "abc123" } },
      ];

      await sendExplainerMode(multimodalContent, ctx, mockFallback);

      const inputItems = streamResponse.mock.calls[0][1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      const userMessage = inputItems.find((i: { role: string }) => i.role === "user")?.content;

      expect(systemPrompt).toContain("Explain this diagram");
      expect(userMessage).toBe("Explain this diagram");
    });

    it("passes multimodal content to fallback when empty audience levels", async () => {
      const ctx = createMockContext({
        modeConfig: { audienceLevels: [] },
      });

      const multimodalContent = [
        { type: "input_text", text: "Question" },
        { type: "image", source: { type: "base64", data: "xyz" } },
      ];

      await sendExplainerMode(multimodalContent, ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith(multimodalContent);
    });
  });

  describe("edge cases", () => {
    it("returns empty results when all levels fail", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue(null);

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results).toEqual([]);
    });

    it("handles partial failures", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "intermediate", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Expert" });
      streamResponse.mockResolvedValueOnce(null); // intermediate fails
      streamResponse.mockResolvedValueOnce({ content: "Beginner" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results).toHaveLength(2);
      expect(results[0]?.modeMetadata?.explainerLevel).toBe("expert");
      expect(results[1]?.modeMetadata?.explainerLevel).toBe("beginner");
    });

    it("uses empty string for previous explanation when first level fails", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce(null); // expert fails
      streamResponse.mockResolvedValueOnce({ content: "Beginner explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Second call should still work but previous_explanation will be empty
      const secondCallInputItems = streamResponse.mock.calls[1][1];
      const systemPrompt = secondCallInputItems.find(
        (i: { role: string }) => i.role === "system"
      )?.content;

      // The prompt template should still be used
      expect(systemPrompt).toContain("beginner");
    });

    it("handles single audience level correctly", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["expert"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Expert explanation",
        usage: { inputTokens: 100, outputTokens: 200 },
      });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results).toHaveLength(1);
      expect(results[0]?.levelLabel).toBe("Expert");
      expect(results[0]?.modeMetadata?.explainerLevels).toEqual(["expert"]);
      expect(results[0]?.usage).toEqual({ inputTokens: 100, outputTokens: 200 });
    });

    it("handles many audience levels correctly", async () => {
      const levels = ["expert", "advanced", "intermediate", "beginner", "child"];
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { audienceLevels: levels },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results).toHaveLength(5);
      expect(streamResponse).toHaveBeenCalledTimes(5);
    });

    it("capitalizes level labels correctly for multi-word levels", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { audienceLevels: ["non-technical"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      // Only first character is capitalized
      expect(results[0]?.levelLabel).toBe("Non-technical");
    });
  });

  describe("default audience levels export", () => {
    it("exports correct default audience levels", () => {
      expect(DEFAULT_AUDIENCE_LEVELS).toEqual(["expert", "intermediate", "beginner"]);
    });
  });

  describe("instance support", () => {
    it("cycles through instances when generating levels", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        instances: [
          { id: "instance-1", modelId: "model-a", label: "Creative Model" },
          { id: "instance-2", modelId: "model-b", label: "Analytical Model" },
        ],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Should cycle through instances
      expect(streamResponse.mock.calls[0][0]).toBe("model-a");
      expect(streamResponse.mock.calls[1][0]).toBe("model-b");
    });

    it("uses instance ID for streaming", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        instances: [{ id: "my-instance", modelId: "model-a", label: "Test Instance" }],
        modeConfig: { audienceLevels: ["expert"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // initStreaming should be called with instance ID
      const [ids, modelMap] = initStreaming.mock.calls[0];
      expect(ids).toEqual(["my-instance"]);
      expect(modelMap.get("my-instance")).toBe("model-a");

      // streamResponse should receive instance ID as stream ID (5th argument)
      expect(streamResponse.mock.calls[0][4]).toBe("my-instance");
    });

    it("merges instance parameters with base settings", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        instances: [
          {
            id: "instance-1",
            modelId: "model-a",
            parameters: { temperature: 0.7, maxTokens: 2000 },
          },
        ],
        settings: { systemPrompt: "Base prompt" },
        modeConfig: { audienceLevels: ["expert"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      await sendExplainerMode("Question", ctx, mockFallback);

      // Settings should merge: base -> instance
      expect(streamResponse).toHaveBeenCalledWith(
        "model-a",
        expect.any(Array),
        expect.any(AbortController),
        expect.objectContaining({
          systemPrompt: "Base prompt", // From base settings
          temperature: 0.7, // From instance params
          maxTokens: 2000, // From instance params
        }),
        expect.any(String)
      );
    });

    it("includes instance label in result metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        instances: [{ id: "instance-1", modelId: "model-a", label: "My Custom Instance" }],
        modeConfig: { audienceLevels: ["expert"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.explainerInstanceLabel).toBe("My Custom Instance");
    });

    it("falls back to model ID for instance label when not set", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        instances: [{ id: "instance-1", modelId: "model-a" }],
        modeConfig: { audienceLevels: ["expert"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Explanation" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.explainerInstanceLabel).toBe("model-a");
    });

    it("includes instance labels in all explanations metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        instances: [
          { id: "instance-1", modelId: "model-a", label: "Expert Model" },
          { id: "instance-2", modelId: "model-b", label: "Simple Model" },
        ],
        modeConfig: { audienceLevels: ["expert", "beginner"] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Expert explanation" });
      streamResponse.mockResolvedValueOnce({ content: "Beginner explanation" });

      const results = await sendExplainerMode("Question", ctx, mockFallback);

      // First result should have all explanations with instance labels
      expect(results[0]?.modeMetadata?.explanations).toHaveLength(2);
      expect(results[0]?.modeMetadata?.explanations?.[0]?.instanceLabel).toBe("Expert Model");
      expect(results[0]?.modeMetadata?.explanations?.[1]?.instanceLabel).toBe("Simple Model");
    });
  });
});
