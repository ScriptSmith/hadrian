import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendDebatedMode } from "../debated";
import { createMockContext as createBaseContext, createMockFallback } from "./test-utils";

// Debated mode uses 2 models by default
function createMockContext(overrides: Parameters<typeof createBaseContext>[0] = {}) {
  return createBaseContext({
    models: ["model-a", "model-b"],
    ...overrides,
  });
}

describe("sendDebatedMode", () => {
  let mockFallback: ReturnType<typeof createMockFallback>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockFallback = createMockFallback();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode with single model", async () => {
      const ctx = createMockContext({ models: ["model-a"] });

      await sendDebatedMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
      expect(ctx.streamResponse).not.toHaveBeenCalled();
    });

    it("does not fall back with two models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response", usage: {} });

      await sendDebatedMode("Hello", ctx, mockFallback);

      expect(mockFallback).not.toHaveBeenCalled();
      expect(streamResponse).toHaveBeenCalled();
    });
  });

  describe("position assignment", () => {
    it("assigns alternating pro/con positions to models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c", "model-d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Debate topic", ctx, mockFallback);

      // First call should have positions
      expect(setModeState).toHaveBeenCalled();
      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.mode).toBe("debated");
      expect(firstCall.positions["model-a"]).toBe("pro");
      expect(firstCall.positions["model-b"]).toBe("con");
      expect(firstCall.positions["model-c"]).toBe("pro");
      expect(firstCall.positions["model-d"]).toBe("con");
    });
  });

  describe("opening phase", () => {
    it("initializes debate state with opening phase", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Opening argument" });

      await sendDebatedMode("Debate topic", ctx, mockFallback);

      expect(setModeState).toHaveBeenCalled();
      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall).toEqual({
        mode: "debated",
        phase: "opening",
        currentRound: 0,
        totalRounds: 3, // default
        positions: { "model-a": "pro", "model-b": "con" },
        turns: [],
        currentRoundTurns: [],
        summarizerModel: "model-a", // default is first model
        summarizerInstanceId: "model-a",
      });
    });

    it("uses custom debate rounds from config", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 5 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.mode).toBe("debated");
      expect(firstCall.phase).toBe("opening");
      expect(firstCall.totalRounds).toBe(5);
    });

    it("uses custom synthesizer model from config", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { synthesizerModel: "model-b" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.mode).toBe("debated");
      expect(firstCall.summarizerModel).toBe("model-b");
    });

    it("initializes streaming for all models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Instance-aware streaming passes [instanceIds], modelMap
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(
        ["model-a", "model-b", "model-c"],
        new Map([
          ["model-a", "model-a"],
          ["model-b", "model-b"],
          ["model-c", "model-c"],
        ])
      );
    });

    it("gathers opening arguments from all models in parallel", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Opening argument" });

      await sendDebatedMode("What is the best approach?", ctx, mockFallback);

      // Instance-aware streaming: (modelId, inputItems, controller, settings, instanceId, trackToolCalls, onSSEEvent, instanceParams)
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
    });

    it("includes position in opening prompt", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Should we use TypeScript?", ctx, mockFallback);

      // Check first model's call (pro position)
      const firstCall = streamResponse.mock.calls[0];
      const inputItems = firstCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("pro");
      expect(systemPrompt).toContain("Should we use TypeScript?");

      // Check second model's call (con position)
      const secondCall = streamResponse.mock.calls[1];
      const inputItems2 = secondCall[1];
      const systemPrompt2 = inputItems2.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt2).toContain("con");
    });

    it("uses custom debate prompt from config", async () => {
      const customPrompt = "Custom debate prompt for {position}";
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debatePrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      const firstCall = streamResponse.mock.calls[0];
      const inputItems = firstCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toBe(customPrompt);
    });

    it("adds opening turns via updateModeState", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Pro opening", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "Con opening", usage: { inputTokens: 15 } });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // updateModeState is called with an updater function - verify it was called
      expect(updateModeState).toHaveBeenCalled();

      // Simulate what the updater function does by calling it with mock state
      const mockState = {
        mode: "debated" as const,
        phase: "opening" as const,
        currentRound: 0,
        totalRounds: 3,
        positions: { "model-a": "pro", "model-b": "con" },
        turns: [] as Array<{ model: string; position: string; content: string; round: number }>,
        currentRoundTurns: [] as Array<{
          model: string;
          position: string;
          content: string;
          round: number;
        }>,
        summarizerModel: "model-a",
      };

      // The first updateModeState call adds the first turn
      const firstUpdater = updateModeState.mock.calls[0][0];
      const afterFirstTurn = firstUpdater(mockState);
      expect(afterFirstTurn.turns).toHaveLength(1);
      expect(afterFirstTurn.turns[0].content).toBe("Pro opening");
      expect(afterFirstTurn.turns[0].round).toBe(0);
    });
  });

  describe("insufficient opening responses", () => {
    it("returns early with single opening response", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Only one" }).mockResolvedValueOnce(null);

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      // Should mark as done immediately
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");

      // Should return single result
      const nonNullResults = results.filter((r) => r !== null);
      expect(nonNullResults).toHaveLength(1);
      expect(nonNullResults[0]?.content).toBe("Only one");
      expect(nonNullResults[0]?.modeMetadata?.isDebateSummary).toBe(false);
    });

    it("returns all nulls when no opening responses succeed", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue(null);

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      expect(results).toEqual([null, null]);
    });
  });

  describe("rebuttal rounds", () => {
    it("transitions to debating phase for rebuttal rounds", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Should transition to debating phase
      const debatingCall = setModeState.mock.calls.find((call) => call[0].phase === "debating")!;
      expect(debatingCall).toBeDefined();
      expect(debatingCall[0].mode).toBe("debated");
      expect(debatingCall[0].currentRound).toBe(1);
    });

    it("re-initializes streaming for each rebuttal round", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 2 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Opening + 2 rebuttal rounds + summary = 4 calls
      expect(initStreaming).toHaveBeenCalledTimes(4);
    });

    it("creates new abort controllers for each rebuttal round", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Verify abort controllers were updated
      expect(ctx.abortControllersRef.current).toBeDefined();
    });

    it("includes previous round arguments in rebuttal prompt", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // Opening responses
      streamResponse
        .mockResolvedValueOnce({ content: "Pro argument content" })
        .mockResolvedValueOnce({ content: "Con argument content" })
        // Rebuttal responses
        .mockResolvedValueOnce({ content: "Pro rebuttal" })
        .mockResolvedValueOnce({ content: "Con rebuttal" })
        // Summary
        .mockResolvedValueOnce({ content: "Summary" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Check rebuttal calls (3rd and 4th calls)
      const rebuttalCall = streamResponse.mock.calls[2];
      const inputItems = rebuttalCall[1];
      const systemPrompt = inputItems[0].content;
      expect(systemPrompt).toContain("Pro argument content");
      expect(systemPrompt).toContain("Con argument content");
    });

    it("adds rebuttal turns to correct round via updateModeState", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        .mockResolvedValueOnce({ content: "Rebuttal A", usage: { inputTokens: 20 } })
        .mockResolvedValueOnce({ content: "Rebuttal B", usage: { inputTokens: 25 } })
        .mockResolvedValueOnce({ content: "Summary" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // updateModeState should have been called for rebuttal turns
      // There are 4 turns total (2 opening + 2 rebuttal)
      expect(updateModeState.mock.calls.length).toBeGreaterThanOrEqual(4);

      // Verify rebuttal turns are added with round 1
      const mockState = {
        mode: "debated" as const,
        phase: "debating" as const,
        currentRound: 1,
        totalRounds: 1,
        positions: { "model-a": "pro", "model-b": "con" },
        turns: [
          { model: "model-a", position: "pro", content: "Opening A", round: 0 },
          { model: "model-b", position: "con", content: "Opening B", round: 0 },
        ],
        currentRoundTurns: [] as Array<{
          model: string;
          position: string;
          content: string;
          round: number;
        }>,
        summarizerModel: "model-a",
      };

      // The 3rd call should add the first rebuttal turn (round 1)
      const thirdUpdater = updateModeState.mock.calls[2][0];
      const afterRebuttal = thirdUpdater(mockState);
      expect(afterRebuttal.turns).toHaveLength(3);
      expect(afterRebuttal.turns[2].content).toBe("Rebuttal A");
      expect(afterRebuttal.turns[2].round).toBe(1);
    });

    it("handles rebuttal failures gracefully", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        // Rebuttals return null (failed)
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null)
        // Summary still runs
        .mockResolvedValueOnce({ content: "Summary" });

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      // Should still complete with summary
      const nonNullResults = results.filter((r) => r !== null);
      expect(nonNullResults).toHaveLength(1);
    });
  });

  describe("summarizing phase", () => {
    it("transitions to summarizing phase after rebuttals", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      const summarizingCall = setModeState.mock.calls.find(
        (call) => call[0].phase === "summarizing"
      )!;
      expect(summarizingCall).toBeDefined();
      expect(summarizingCall[0].mode).toBe("debated");
      expect(summarizingCall[0].currentRound).toBe(1);
    });

    it("initializes streaming for summarizer model only", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Last init should be for summarizer only
      const lastInitCall = initStreaming.mock.calls[initStreaming.mock.calls.length - 1];
      expect(lastInitCall[0]).toEqual(["model-a"]);
    });

    it("includes full debate transcript in summary prompt", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening pro" })
        .mockResolvedValueOnce({ content: "Opening con" })
        .mockResolvedValueOnce({ content: "Rebuttal pro" })
        .mockResolvedValueOnce({ content: "Rebuttal con" })
        .mockResolvedValueOnce({ content: "Final summary" });

      await sendDebatedMode("Debate question", ctx, mockFallback);

      // Check summary call (last call)
      const summaryCall = streamResponse.mock.calls[4];
      const inputItems = summaryCall[1];
      const systemPrompt = inputItems[0].content;
      expect(systemPrompt).toContain("Debate question");
      expect(systemPrompt).toContain("Opening pro");
      expect(systemPrompt).toContain("Opening con");
      expect(systemPrompt).toContain("Rebuttal pro");
      expect(systemPrompt).toContain("Rebuttal con");
    });

    it("handles summary failure with fallback message", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        .mockRejectedValueOnce(new Error("Summary failed"));

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.content).toContain("could not be summarized");
    });
  });

  describe("result construction", () => {
    it("returns summary as the result", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        .mockResolvedValueOnce({ content: "Final summary content" });

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.content).toBe("Final summary content");
    });

    it("places result at summarizer model index", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-b", debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      // Result should be at index 1 (model-b)
      expect(results[0]).toBeNull();
      expect(results[1]).not.toBeNull();
      expect(results[2]).toBeNull();
    });

    it("includes mode metadata with debate info", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata).toBeDefined();
      expect(result?.modeMetadata?.mode).toBe("debated");
      expect(result?.modeMetadata?.isDebateSummary).toBe(true);
      expect(result?.modeMetadata?.debatePositions).toEqual({
        "model-a": "pro",
        "model-b": "con",
      });
      expect(result?.modeMetadata?.debateTurns).toBeDefined();
      expect(result?.modeMetadata?.debateRounds).toBeDefined();
      expect(result?.modeMetadata?.summarizerModel).toBe("model-a");
    });

    it("includes all debate turns in metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        .mockResolvedValueOnce({ content: "Rebuttal A" })
        .mockResolvedValueOnce({ content: "Rebuttal B" })
        .mockResolvedValueOnce({ content: "Summary" });

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata?.debateTurns).toHaveLength(4);
    });

    it("aggregates usage from all turns and summary", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: { inputTokens: 10, outputTokens: 20 } })
        .mockResolvedValueOnce({ content: "B", usage: { inputTokens: 15, outputTokens: 25 } })
        .mockResolvedValueOnce({ content: "S", usage: { inputTokens: 30, outputTokens: 40 } });

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      // Total: 10+15+30 = 55 input, 20+25+40 = 85 output
      expect(result?.modeMetadata?.aggregateUsage?.inputTokens).toBe(55);
      expect(result?.modeMetadata?.aggregateUsage?.outputTokens).toBe(85);
    });

    it("includes summary usage separately in metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "B", usage: { inputTokens: 15 } })
        .mockResolvedValueOnce({
          content: "Summary",
          usage: { inputTokens: 50, outputTokens: 100 },
        });

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata?.summaryUsage).toEqual({ inputTokens: 50, outputTokens: 100 });
    });

    it("calculates debateRounds including opening round", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 2 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendDebatedMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      // Opening (1) + rebuttal rounds (2) = 3
      expect(result?.modeMetadata?.debateRounds).toBe(3);
    });
  });

  describe("state transitions", () => {
    it("transitions through opening -> debating -> summarizing -> done phases", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 1 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      const phases = setModeState.mock.calls.map((call) => call[0].phase);
      expect(phases[0]).toBe("opening");
      expect(phases).toContain("debating");
      expect(phases).toContain("summarizing");
      expect(phases[phases.length - 1]).toBe("done");
    });

    it("skips debating phase when debateRounds is 0", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      const phases = setModeState.mock.calls.map((call) => call[0].phase);
      expect(phases).not.toContain("debating");
      expect(phases).toEqual(["opening", "summarizing", "done"]);
    });

    it("sets final state with summary content and usage", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A" })
        .mockResolvedValueOnce({ content: "B" })
        .mockResolvedValueOnce({ content: "Final summary", usage: { inputTokens: 50 } });

      await sendDebatedMode("Topic", ctx, mockFallback);

      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.summary).toBe("Final summary");
      expect(lastCall.summaryUsage).toEqual({ inputTokens: 50 });
    });
  });

  describe("abort controller management", () => {
    it("creates abort controllers for all models in opening", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Instance-aware streaming: (modelId, inputItems, controller, settings, instanceId, trackToolCalls, onSSEEvent, instanceParams)
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

      await sendDebatedMode("Topic", ctx, mockFallback);

      // After completion, ref should have the summary controller
      expect(ctx.abortControllersRef.current).toBeDefined();
      expect(ctx.abortControllersRef.current.length).toBeGreaterThanOrEqual(1);
    });

    it("creates separate abort controller for summary phase", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Summary uses single abort controller
      expect(ctx.abortControllersRef.current).toHaveLength(1);
    });
  });

  describe("message filtering", () => {
    it("filters messages for each model in opening round", async () => {
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

      await sendDebatedMode("Topic", ctx, mockFallback);

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-a");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-b");
    });
  });

  describe("multimodal content handling", () => {
    it("extracts text from multimodal content for prompts", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const multimodalContent = [
        { type: "input_text", text: "Debate this image" },
        { type: "image", source: { type: "base64", data: "abc123" } },
      ];

      await sendDebatedMode(multimodalContent, ctx, mockFallback);

      // Check opening prompt contains extracted text
      const firstCall = streamResponse.mock.calls[0];
      const inputItems = firstCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("Debate this image");
    });

    it("passes multimodal content to fallback when needed", async () => {
      const ctx = createMockContext({ models: ["model-a"] });

      const multimodalContent = [
        { type: "input_text", text: "Debate topic" },
        { type: "image", source: { type: "base64", data: "xyz" } },
      ];

      await sendDebatedMode(multimodalContent, ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith(multimodalContent);
    });
  });

  describe("multiple rebuttal rounds", () => {
    it("runs correct number of rebuttal rounds", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 3 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Count debating phase calls
      const debatingCalls = setModeState.mock.calls.filter((call) => call[0].phase === "debating");
      expect(debatingCalls).toHaveLength(3);

      // Verify round numbers
      expect(debatingCalls[0][0].currentRound).toBe(1);
      expect(debatingCalls[1][0].currentRound).toBe(2);
      expect(debatingCalls[2][0].currentRound).toBe(3);
    });

    it("accumulates turns across all rounds", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        modeConfig: { debateRounds: 2 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendDebatedMode("Topic", ctx, mockFallback);

      // Opening (2) + Round 1 (2) + Round 2 (2) = 6 turns
      expect(updateModeState).toHaveBeenCalledTimes(6);
    });
  });
});
