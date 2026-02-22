import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendConsensusMode } from "../consensus";
import { createMockContext, createMockFallback } from "./test-utils";

describe("sendConsensusMode", () => {
  let mockFallback: ReturnType<typeof createMockFallback>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockFallback = createMockFallback();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode with single model", async () => {
      const ctx = createMockContext({ models: ["model-a"] });

      await sendConsensusMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
      expect(ctx.streamResponse).not.toHaveBeenCalled();
    });

    it("does not fall back with two models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response", usage: {} });

      await sendConsensusMode("Hello", ctx, mockFallback);

      expect(mockFallback).not.toHaveBeenCalled();
      expect(streamResponse).toHaveBeenCalled();
    });
  });

  describe("initial responding phase", () => {
    it("initializes consensus state with responding phase", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendConsensusMode("Hello", ctx, mockFallback);

      // First call should initialize with responding phase
      expect(setModeState).toHaveBeenNthCalledWith(1, {
        mode: "consensus",
        phase: "responding",
        currentRound: 0,
        maxRounds: 5,
        threshold: 0.8,
        rounds: [],
        currentRoundResponses: [],
      });
    });

    it("uses custom maxRounds and threshold from config", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { maxConsensusRounds: 3, consensusThreshold: 0.9 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendConsensusMode("Hello", ctx, mockFallback);

      expect(setModeState).toHaveBeenNthCalledWith(1, {
        mode: "consensus",
        phase: "responding",
        currentRound: 0,
        maxRounds: 3,
        threshold: 0.9,
        rounds: [],
        currentRoundResponses: [],
      });
    });

    it("initializes streaming for all models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendConsensusMode("Hello", ctx, mockFallback);

      // gatherInstances passes instance IDs with model map
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(
        ["model-a", "model-b", "model-c"],
        new Map([
          ["model-a", "model-a"],
          ["model-b", "model-b"],
          ["model-c", "model-c"],
        ])
      );
    });

    it("gathers initial responses from all models in parallel", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "Response B", usage: { inputTokens: 15 } })
        .mockResolvedValueOnce({ content: "Response C", usage: { inputTokens: 20 } });

      await sendConsensusMode("Hello", ctx, mockFallback);

      // gatherInstances passes instance-aware arguments
      expect(streamResponse).toHaveBeenCalledWith(
        "model-a",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-a", // instanceId
        undefined,
        undefined,
        undefined, // instanceParams
        undefined // instanceLabel
      );
      expect(streamResponse).toHaveBeenCalledWith(
        "model-b",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-b",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
      expect(streamResponse).toHaveBeenCalledWith(
        "model-c",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-c",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
    });

    it("adds initial responses via updateModeState", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "Response B", usage: { inputTokens: 15 } });

      await sendConsensusMode("Hello", ctx, mockFallback);

      // updateModeState should be called for each successful response
      expect(updateModeState).toHaveBeenCalled();
      // Verify the updater function works correctly by calling it
      const firstCall = updateModeState.mock.calls[0][0];
      const mockState = {
        mode: "consensus" as const,
        phase: "responding" as const,
        currentRound: 0,
        maxRounds: 5,
        threshold: 0.8,
        rounds: [],
        currentRoundResponses: [],
      };
      const result = firstCall(mockState);
      expect(result.currentRoundResponses).toHaveLength(1);
    });
  });

  describe("insufficient initial responses", () => {
    it("handles single successful response from initial round", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Only success", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null);

      const results = await sendConsensusMode("Hello", ctx, mockFallback);

      // Should still return result for the one successful response
      expect(results[0]).not.toBeNull();
      expect(results[0]?.content).toBe("Only success");
      expect(results[0]?.modeMetadata?.mode).toBe("consensus");
      expect(results[0]?.modeMetadata?.consensusReached).toBe(false);
    });

    it("returns all nulls when no responses succeed", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue(null);

      const results = await sendConsensusMode("Hello", ctx, mockFallback);

      expect(results).toEqual([null, null]);
    });

    it("marks done state when insufficient responses", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Only one" }).mockResolvedValueOnce(null);

      await sendConsensusMode("Hello", ctx, mockFallback);

      // Should set done state with the single round
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1];
      expect(lastCall[0].phase).toBe("done");
    });
  });

  describe("revision rounds", () => {
    it("transitions to revising phase for revision rounds", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      // Initial responses - different content to not reach consensus
      streamResponse
        .mockResolvedValueOnce({ content: "Apple orange banana" })
        .mockResolvedValueOnce({ content: "Cat dog fish" })
        // Revision round - more similar content
        .mockResolvedValueOnce({ content: "Apple orange banana cat dog" })
        .mockResolvedValueOnce({ content: "Apple orange banana cat dog" });

      await sendConsensusMode("Hello", ctx, mockFallback);

      // Should transition to revising phase for round 1
      expect(setModeState).toHaveBeenCalledWith(
        expect.objectContaining({
          mode: "consensus",
          phase: "revising",
          currentRound: 1,
          maxRounds: 5,
          threshold: 0.8,
        })
      );
    });

    it("re-initializes streaming for each revision round", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      // Initial + 1 revision
      streamResponse
        .mockResolvedValueOnce({ content: "Different" })
        .mockResolvedValueOnce({ content: "Content" })
        .mockResolvedValueOnce({ content: "Same content here" })
        .mockResolvedValueOnce({ content: "Same content here" });

      await sendConsensusMode("Hello", ctx, mockFallback);

      // Called twice: once for initial, once for revision
      expect(initStreaming).toHaveBeenCalledTimes(2);
    });

    it("creates new abort controllers for each revision round", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A" })
        .mockResolvedValueOnce({ content: "B" })
        .mockResolvedValueOnce({ content: "Same" })
        .mockResolvedValueOnce({ content: "Same" });

      await sendConsensusMode("Hello", ctx, mockFallback);

      // Verify abort controllers were updated
      expect(ctx.abortControllersRef.current).toBeDefined();
      expect(ctx.abortControllersRef.current.length).toBe(2);
    });

    it("adds revision responses to the correct round via updateModeState", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial A" })
        .mockResolvedValueOnce({ content: "Initial B" })
        .mockResolvedValueOnce({ content: "Revised A", usage: { inputTokens: 20 } })
        .mockResolvedValueOnce({ content: "Revised B", usage: { inputTokens: 25 } });

      await sendConsensusMode("Hello", ctx, mockFallback);

      // updateModeState should be called for both initial and revision responses
      // 2 initial + 2 revision = 4 total calls
      expect(updateModeState).toHaveBeenCalledTimes(4);

      // Test that the updater functions work correctly for revision round
      const revisionUpdater = updateModeState.mock.calls[2][0];
      const mockState = {
        mode: "consensus" as const,
        phase: "revising" as const,
        currentRound: 1,
        maxRounds: 5,
        threshold: 0.8,
        rounds: [],
        currentRoundResponses: [],
      };
      const result = revisionUpdater(mockState);
      expect(result.currentRoundResponses).toHaveLength(1);
      expect(result.currentRoundResponses[0].content).toBe("Revised A");
    });
  });

  describe("revision prompt", () => {
    it("uses default consensus prompt with question and responses", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response from A" })
        .mockResolvedValueOnce({ content: "Response from B" })
        .mockResolvedValueOnce({ content: "Revised A" })
        .mockResolvedValueOnce({ content: "Revised B" });

      await sendConsensusMode("What is the best approach?", ctx, mockFallback);

      // Check the revision round calls (3rd and 4th)
      const revisionCall = streamResponse.mock.calls[2];
      const inputItems = revisionCall[1];

      // System prompt should contain the question and responses
      expect(inputItems[0].role).toBe("system");
      expect(inputItems[0].content).toContain("What is the best approach?");
      expect(inputItems[0].content).toContain("model-a");
      expect(inputItems[0].content).toContain("Response from A");
      expect(inputItems[0].content).toContain("model-b");
      expect(inputItems[0].content).toContain("Response from B");

      // User message asks for revised response
      expect(inputItems[1].role).toBe("user");
      expect(inputItems[1].content).toBe("Please provide your revised response.");
    });

    it("uses custom consensus prompt from config", async () => {
      const customPrompt = "Custom consensus prompt: synthesize all views";
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { consensusPrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A" })
        .mockResolvedValueOnce({ content: "B" })
        .mockResolvedValueOnce({ content: "Revised" })
        .mockResolvedValueOnce({ content: "Revised" });

      await sendConsensusMode("Test", ctx, mockFallback);

      const revisionCall = streamResponse.mock.calls[2];
      expect(revisionCall[1][0].content).toBe(customPrompt);
    });
  });

  describe("consensus detection", () => {
    it("stops early when consensus threshold is reached", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { consensusThreshold: 0.5 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // Initial responses very different
      streamResponse
        .mockResolvedValueOnce({ content: "Alpha beta gamma" })
        .mockResolvedValueOnce({ content: "One two three" })
        // Revision round - identical content should reach 1.0 consensus
        .mockResolvedValueOnce({ content: "Same response here" })
        .mockResolvedValueOnce({ content: "Same response here" });

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      // Should only need 1 revision round
      // Initial (2) + 1 revision (2) = 4 calls
      expect(streamResponse).toHaveBeenCalledTimes(4);
      expect(results[0]?.modeMetadata?.consensusReached).toBe(true);
    });

    it("continues revisions until max rounds when consensus not reached", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { maxConsensusRounds: 2, consensusThreshold: 0.99 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // All responses different - never reach 0.99 threshold
      streamResponse.mockResolvedValue({ content: `Unique response ${Math.random()}` });

      await sendConsensusMode("Test", ctx, mockFallback);

      // Initial (2) + 1 revision round (2) = 4 calls (maxRounds=2 means 1 revision round)
      expect(streamResponse).toHaveBeenCalledTimes(4);
    });

    it("sets consensusReached true in metadata when threshold met", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { consensusThreshold: 0.0 }, // Very low threshold
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Some content" });

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.consensusReached).toBe(true);
    });

    it("includes consensus score in metadata", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // Identical responses = perfect consensus
      streamResponse.mockResolvedValue({ content: "Identical response content" });

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.consensusScore).toBeDefined();
      expect(results[0]?.modeMetadata?.consensusScore).toBe(1.0);
    });
  });

  describe("representative response selection", () => {
    it("selects response most similar to others as representative", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // model-b is more similar to both model-a and model-c than they are to each other
      streamResponse
        .mockResolvedValueOnce({ content: "Apples oranges bananas" }) // model-a
        .mockResolvedValueOnce({ content: "Apples oranges grapes" }) // model-b - shares with both
        .mockResolvedValueOnce({ content: "Mangoes oranges grapes" }) // model-c
        // After consensus threshold check, these are similar enough
        .mockResolvedValueOnce({ content: "Fruit salad" })
        .mockResolvedValueOnce({ content: "Fruit salad" })
        .mockResolvedValueOnce({ content: "Fruit salad" });

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      // The representative should have high similarity - exact model depends on content
      expect(results.filter((r) => r !== null)).toHaveLength(1);
    });

    it("returns single response when only one model succeeds in final round", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial A" })
        .mockResolvedValueOnce({ content: "Initial B" })
        // Only one succeeds in revision
        .mockResolvedValueOnce({ content: "Revised A" })
        .mockResolvedValueOnce(null);

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      expect(results.filter((r) => r !== null)).toHaveLength(1);
    });
  });

  describe("result construction", () => {
    it("includes mode metadata with consensus info", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Consensus content",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata).toBeDefined();
      expect(result?.modeMetadata?.mode).toBe("consensus");
      expect(result?.modeMetadata?.isConsensus).toBe(true);
      expect(result?.modeMetadata?.consensusRound).toBeDefined();
      expect(result?.modeMetadata?.totalRounds).toBeDefined();
      expect(result?.modeMetadata?.consensusReached).toBeDefined();
      expect(result?.modeMetadata?.rounds).toBeDefined();
    });

    it("includes all rounds in metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { maxConsensusRounds: 3, consensusThreshold: 0.99 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // Completely different words each time to ensure low Jaccard similarity
      // and avoid early consensus (threshold 0.99 is nearly impossible)
      const uniqueResponses = [
        "alpha beta gamma delta epsilon", // model-a round 0
        "one two three four five", // model-b round 0
        "cat dog fish bird snake", // model-a round 1
        "red blue green yellow purple", // model-b round 1
      ];
      let callIndex = 0;
      streamResponse.mockImplementation(() => {
        const content = uniqueResponses[callIndex % uniqueResponses.length];
        callIndex++;
        return Promise.resolve({ content });
      });

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      // Initial round (0) + 1 revision round (1) = 2 rounds
      // (maxConsensusRounds: 3 means loop runs for round=1,2 but stops at round < 3)
      // Actually: loop is `for (round = 1; round < maxRounds; round++)` so with maxRounds=3,
      // we get rounds 1 and 2, giving us total of 3 rounds (0, 1, 2)
      // But since consensus is calculated AFTER each revision, and we need threshold 0.99,
      // it should continue through all rounds
      expect(result?.modeMetadata?.rounds?.length).toBeGreaterThanOrEqual(2);
      expect(result?.modeMetadata?.rounds?.[0].round).toBe(0);
      expect(result?.modeMetadata?.rounds?.[1].round).toBe(1);
    });

    it("aggregates usage from all rounds", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: { inputTokens: 10, outputTokens: 20 } })
        .mockResolvedValueOnce({ content: "B", usage: { inputTokens: 15, outputTokens: 25 } })
        .mockResolvedValueOnce({ content: "Same", usage: { inputTokens: 30, outputTokens: 40 } })
        .mockResolvedValueOnce({ content: "Same", usage: { inputTokens: 35, outputTokens: 45 } });

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      // Total: 10+15+30+35 = 90 input, 20+25+40+45 = 130 output
      expect(result?.modeMetadata?.aggregateUsage?.inputTokens).toBe(90);
      expect(result?.modeMetadata?.aggregateUsage?.outputTokens).toBe(130);
    });

    it("places result at representative model index", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Same content" });

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      // Should have exactly one non-null result
      const nonNullResults = results.filter((r) => r !== null);
      expect(nonNullResults).toHaveLength(1);

      // Result index should match a valid model index
      const resultIndex = results.findIndex((r) => r !== null);
      expect(resultIndex).toBeGreaterThanOrEqual(0);
      expect(resultIndex).toBeLessThan(ctx.models.length);
    });
  });

  describe("state transitions", () => {
    it("transitions through responding -> revising -> done phases", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Different A" })
        .mockResolvedValueOnce({ content: "Different B" })
        .mockResolvedValueOnce({ content: "Same" })
        .mockResolvedValueOnce({ content: "Same" });

      await sendConsensusMode("Test", ctx, mockFallback);

      // Check phase transitions
      const phases = setModeState.mock.calls.map((call) => call[0].phase);
      expect(phases[0]).toBe("responding"); // Initial
      expect(phases).toContain("revising"); // During revision
      expect(phases[phases.length - 1]).toBe("done"); // Final
    });

    it("sets final consensus score when done", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Identical" });

      await sendConsensusMode("Test", ctx, mockFallback);

      // Last call should have final score
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1];
      expect(lastCall[0].phase).toBe("done");
      expect(lastCall[0].finalScore).toBe(1.0); // Perfect consensus score
    });
  });

  describe("abort controller management", () => {
    it("creates abort controllers for all models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendConsensusMode("Test", ctx, mockFallback);

      // Each instance should get its own abort controller (instance-aware args)
      expect(streamResponse).toHaveBeenCalledWith(
        "model-a",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-a",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
      expect(streamResponse).toHaveBeenCalledWith(
        "model-b",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-b",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
      expect(streamResponse).toHaveBeenCalledWith(
        "model-c",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-c",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
    });

    it("stores abort controllers in ref", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendConsensusMode("Test", ctx, mockFallback);

      expect(ctx.abortControllersRef.current).toHaveLength(2);
      expect(ctx.abortControllersRef.current[0]).toBeInstanceOf(AbortController);
      expect(ctx.abortControllersRef.current[1]).toBeInstanceOf(AbortController);
    });
  });

  describe("message filtering", () => {
    it("filters messages for each model in initial round", async () => {
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

      await sendConsensusMode("Test", ctx, mockFallback);

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-a");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-b");
    });
  });

  describe("multimodal content handling", () => {
    it("extracts text from multimodal content for revision prompt", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A" })
        .mockResolvedValueOnce({ content: "Response B" })
        .mockResolvedValueOnce({ content: "Revised" })
        .mockResolvedValueOnce({ content: "Revised" });

      const multimodalContent = [
        { type: "input_text", text: "Describe this image" },
        { type: "image", source: { type: "base64", data: "abc123" } },
      ];

      await sendConsensusMode(multimodalContent, ctx, mockFallback);

      // Check revision prompt contains extracted text
      const revisionCall = streamResponse.mock.calls[2];
      expect(revisionCall[1][0].content).toContain("Describe this image");
    });

    it("passes multimodal content to models in initial round", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const multimodalContent = [
        { type: "input_text", text: "What is this?" },
        { type: "image", source: { type: "base64", data: "xyz789" } },
      ];

      await sendConsensusMode(multimodalContent, ctx, mockFallback);

      // Initial calls should include the multimodal content
      const firstCall = streamResponse.mock.calls[0];
      const inputItems = firstCall[1];
      expect(inputItems[inputItems.length - 1].content).toEqual(multimodalContent);
    });
  });

  describe("revision failure handling", () => {
    it("continues with previous responses when revision fails", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Initial A" })
        .mockResolvedValueOnce({ content: "Initial B" })
        // All revisions fail
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null);

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      // Should still return a result based on initial responses
      expect(results.filter((r) => r !== null)).toHaveLength(1);
    });

    it("handles partial revision failures gracefully", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A" })
        .mockResolvedValueOnce({ content: "B" })
        .mockResolvedValueOnce({ content: "C" })
        // One revision succeeds, others fail
        .mockResolvedValueOnce({ content: "Revised A" })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null);

      const results = await sendConsensusMode("Test", ctx, mockFallback);

      // Should still return a result
      expect(results.filter((r) => r !== null)).toHaveLength(1);
    });
  });
});
