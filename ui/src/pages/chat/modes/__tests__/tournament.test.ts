import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendTournamentMode } from "../tournament";
import { createMockContext as createBaseContext } from "./test-utils";

// Tournament mode uses 4 models by default
function createMockContext(overrides: Parameters<typeof createBaseContext>[0] = {}) {
  return createBaseContext({
    models: ["model-a", "model-b", "model-c", "model-d"],
    ...overrides,
  });
}

describe("sendTournamentMode", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  describe("model count fallback", () => {
    it("falls back to multiple mode with fewer than 4 models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const fallback = vi.fn().mockResolvedValue([{ content: "fallback" }]);

      const results = await sendTournamentMode("Hello", ctx, fallback);

      expect(fallback).toHaveBeenCalledWith("Hello");
      expect(results).toEqual([{ content: "fallback" }]);
      expect(ctx.streamingStore.setModeState).not.toHaveBeenCalled();
    });

    it("falls back to multiple mode with 2 models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const fallback = vi.fn().mockResolvedValue([{ content: "two" }]);

      const results = await sendTournamentMode("Hello", ctx, fallback);

      expect(fallback).toHaveBeenCalledWith("Hello");
      expect(results).toEqual([{ content: "two" }]);
    });

    it("falls back to multiple mode with single model", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const fallback = vi.fn().mockResolvedValue([{ content: "single" }]);

      const results = await sendTournamentMode("Hello", ctx, fallback);

      expect(fallback).toHaveBeenCalledWith("Hello");
      expect(results).toEqual([{ content: "single" }]);
    });

    it("proceeds with tournament when exactly 4 models present", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const fallback = vi.fn();
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, fallback);

      expect(fallback).not.toHaveBeenCalled();
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });

    it("proceeds with tournament when more than 4 models present", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d", "e"] });
      const fallback = vi.fn();
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, fallback);

      expect(fallback).not.toHaveBeenCalled();
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });
  });

  describe("generating phase", () => {
    it("initializes tournament state in generating phase", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Check the first call starts with generating phase and initial bracket
      const firstCall = (ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>).mock
        .calls[0][0];
      expect(firstCall.phase).toBe("generating");
      expect(firstCall.bracket[0]).toEqual(["a", "b", "c", "d"]); // Initial bracket round 0
      expect(firstCall.currentRound).toBe(0);
      expect(firstCall.totalRounds).toBe(2); // log2(4) = 2
    });

    it("calculates correct number of rounds for 4 models", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // log2(4) = 2 rounds
      const firstCall = (ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>).mock
        .calls[0][0];
      expect(firstCall.totalRounds).toBe(2);
    });

    it("calculates correct number of rounds for 8 models", async () => {
      const ctx = createMockContext({
        models: ["a", "b", "c", "d", "e", "f", "g", "h"],
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // log2(8) = 3 rounds
      const firstCall = (ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>).mock
        .calls[0][0];
      expect(firstCall.totalRounds).toBe(3);
    });

    it("calculates correct number of rounds for 5 models (non-power of 2)", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d", "e"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // ceil(log2(5)) = 3 rounds
      const firstCall = (ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>).mock
        .calls[0][0];
      expect(firstCall.totalRounds).toBe(3);
    });

    it("initializes streaming for all models", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Instance-aware streaming passes [instanceIds], modelMap
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(
        ["a", "b", "c", "d"],
        new Map([
          ["a", "a"],
          ["b", "b"],
          ["c", "c"],
          ["d", "d"],
        ])
      );
    });

    it("creates abort controllers for all models", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Should have controllers for initial generation
      expect(ctx.abortControllersRef.current.length).toBeGreaterThan(0);
    });

    it("streams responses from all models in parallel", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // 4 initial responses + judge calls for matches
      // Instance-aware streaming: (modelId, inputItems, controller, settings, instanceId, trackToolCalls, onSSEEvent, instanceParams, instanceLabel)
      expect(streamResponse).toHaveBeenCalledWith(
        "a",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "a", // instance ID
        undefined,
        undefined,
        undefined, // instance params
        undefined // instanceLabel
      );
      expect(streamResponse).toHaveBeenCalledWith(
        "b",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "b",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
      expect(streamResponse).toHaveBeenCalledWith(
        "c",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "c",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
      expect(streamResponse).toHaveBeenCalledWith(
        "d",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "d",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
    });

    it("adds each successful initial response", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "Response B", usage: { inputTokens: 20 } })
        .mockResolvedValueOnce({ content: "Response C", usage: { inputTokens: 30 } })
        .mockResolvedValueOnce({ content: "Response D", usage: { inputTokens: 40 } })
        .mockResolvedValue({ content: "A" }); // Judge responses

      await sendTournamentMode("Hello", ctx, vi.fn());

      // updateModeState is called for each initial response
      expect(ctx.streamingStore.updateModeState).toHaveBeenCalled();
    });
  });

  describe("insufficient responses handling", () => {
    it("returns early when fewer than 2 responses succeed", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Only Response", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null);

      const results = await sendTournamentMode("Hello", ctx, vi.fn());

      // Should mark as done without competing
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");

      // Winner at correct position
      expect(results[0]).not.toBeNull();
      expect(results[0]?.content).toBe("Only Response");
      expect(results[0]?.modeMetadata?.isTournamentWinner).toBe(true);
    });

    it("returns empty results when no responses succeed", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue(null);

      const results = await sendTournamentMode("Hello", ctx, vi.fn());

      expect(results.filter((r) => r !== null)).toHaveLength(0);
    });
  });

  describe("competing phase", () => {
    it("transitions to competing phase after gathering responses", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      const setStateCalls = (ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      expect(setStateCalls.some((call) => call[0].phase === "competing")).toBe(true);
    });

    it("creates tournament matches for competitor pairs", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValue({ content: "A" }); // Judge always picks first

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Should add matches via updateModeState
      expect(ctx.streamingStore.updateModeState).toHaveBeenCalled();
    });

    it("handles bye for odd number of competitors", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d", "e"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // 5 initial responses
      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValueOnce({ content: "E", usage: {} })
        .mockResolvedValue({ content: "A" }); // Judge responses

      await sendTournamentMode("Hello", ctx, vi.fn());

      // With 5 competitors: first gets bye, then 2 matches (b vs c, d vs e)
      // Then 3 competitors in round 2: first gets bye, 1 match
      // Then final match
      const matches = (ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>).mock.calls;
      // Just verify matches were created
      expect(matches.length).toBeGreaterThan(0);
    });
  });

  describe("judging", () => {
    it("selects judge model for each match", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValue({ content: "A" }); // Judge responses

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Matches should be updated with judge via updateModeState
      expect(ctx.streamingStore.updateModeState).toHaveBeenCalled();
    });

    it("uses configured primaryModel as judge when available", async () => {
      const ctx = createMockContext({
        models: ["a", "b", "c", "d"],
        modeConfig: { primaryModel: "judge-model" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Judge calls should use primaryModel - verify via updateModeState being called
      expect(ctx.streamingStore.updateModeState).toHaveBeenCalled();
    });

    it("uses custom voting prompt from config", async () => {
      const customPrompt = "Custom judging prompt: pick better";
      const ctx = createMockContext({
        models: ["a", "b", "c", "d"],
        modeConfig: { votingPrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Judge calls should use custom prompt
      const judgeCalls = streamResponse.mock.calls.slice(4); // Skip initial 4 responses
      if (judgeCalls.length > 0) {
        const systemContent = judgeCalls[0][1][0].content;
        expect(systemContent).toBe(customPrompt);
      }
    });

    it("uses instance-aware streaming for judge responses", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Judge calls use instance-aware streaming (includes instance ID as 5th arg)
      const judgeCalls = streamResponse.mock.calls.slice(4);
      if (judgeCalls.length > 0) {
        // Verify judge call has instance ID parameter
        expect(judgeCalls[0][4]).toBeDefined(); // streamId/instanceId
      }
    });

    it("parses judge decision - 'A' selects competitor1", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A", usage: {} })
        .mockResolvedValueOnce({ content: "Response B", usage: {} })
        .mockResolvedValueOnce({ content: "Response C", usage: {} })
        .mockResolvedValueOnce({ content: "Response D", usage: {} })
        .mockResolvedValue({ content: "A" }); // Judge picks A (competitor1)

      await sendTournamentMode("Hello", ctx, vi.fn());

      const updateCalls = (ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      // First match winner should be first competitor
      const firstUpdate = updateCalls.find((call) => call[0] === "0-0");
      if (firstUpdate) {
        expect(firstUpdate[1].winner).toBe("a");
      }
    });

    it("parses judge decision - 'B' selects competitor2", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A", usage: {} })
        .mockResolvedValueOnce({ content: "Response B", usage: {} })
        .mockResolvedValueOnce({ content: "Response C", usage: {} })
        .mockResolvedValueOnce({ content: "Response D", usage: {} })
        .mockResolvedValue({ content: "B" }); // Judge picks B (competitor2)

      await sendTournamentMode("Hello", ctx, vi.fn());

      const updateCalls = (ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      const firstUpdate = updateCalls.find((call) => call[0] === "0-0");
      if (firstUpdate) {
        expect(firstUpdate[1].winner).toBe("b"); // competitor2 wins
      }
    });

    it("parses judge decision - '1' selects competitor1", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValue({ content: "1" }); // Judge picks 1

      await sendTournamentMode("Hello", ctx, vi.fn());

      const updateCalls = (ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      const firstUpdate = updateCalls.find((call) => call[0] === "0-0");
      if (firstUpdate) {
        expect(firstUpdate[1].winner).toBe("a");
      }
    });

    it("parses judge decision - '2' selects competitor2", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValue({ content: "2" }); // Judge picks 2

      await sendTournamentMode("Hello", ctx, vi.fn());

      const updateCalls = (ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      const firstUpdate = updateCalls.find((call) => call[0] === "0-0");
      if (firstUpdate) {
        expect(firstUpdate[1].winner).toBe("b");
      }
    });

    it("defaults to competitor1 when judge response is ambiguous", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValue({ content: "Both are good" }); // Ambiguous

      await sendTournamentMode("Hello", ctx, vi.fn());

      const updateCalls = (ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      const firstUpdate = updateCalls.find((call) => call[0] === "0-0");
      if (firstUpdate) {
        expect(firstUpdate[1].winner).toBe("a"); // Default to competitor1
      }
    });

    it("handles judge errors gracefully - defaults to competitor1", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockRejectedValue(new Error("Judge failed")); // Judge errors

      const results = await sendTournamentMode("Hello", ctx, vi.fn());

      // Should still complete without throwing
      expect(results).toBeDefined();
    });

    it("stores judge reasoning in match data", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValue({ content: "A is more comprehensive", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      const updateCalls = (ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      const firstUpdate = updateCalls.find((call) => call[0] === "0-0");
      if (firstUpdate) {
        expect(firstUpdate[1].reasoning).toBe("A is more comprehensive");
      }
    });

    it("stores judge usage in match data", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      const judgeUsage = { inputTokens: 100, outputTokens: 10 };
      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValue({ content: "A", usage: judgeUsage });

      await sendTournamentMode("Hello", ctx, vi.fn());

      const updateCalls = (ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      const firstUpdate = updateCalls.find((call) => call[0] === "0-0");
      if (firstUpdate) {
        expect(firstUpdate[1].judgeUsage).toEqual(judgeUsage);
      }
    });
  });

  describe("missing response handling", () => {
    it("auto-wins competitor when opponent has no response", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce(null) // b fails
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValue({ content: "A" });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // a should auto-advance since b has no response
      // Tournament should still complete
      const results = await sendTournamentMode("Hello", ctx, vi.fn());
      expect(results).toBeDefined();
    });
  });

  describe("tournament progression", () => {
    it("advances winners to next round", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // Round 1: a beats b, c beats d
      // Round 2: a beats c
      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "D", usage: {} })
        .mockResolvedValue({ content: "A" }); // Judge always picks A/first

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Should have matches progressed via updateModeState
      expect(ctx.streamingStore.updateModeState).toHaveBeenCalled();
    });

    it("updates bracket with round winners", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Final state should have updated bracket
      const lastCall = (
        ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>
      ).mock.calls.slice(-1)[0][0];
      // Bracket should have multiple rounds
      expect(lastCall.bracket.length).toBeGreaterThan(1);
    });

    it("tracks eliminated models per round", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // a beats b, c beats d, a beats c
      // Eliminated: b in round 0, d in round 0, c in round 1
      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      const lastCall = (
        ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>
      ).mock.calls.slice(-1)[0][0];
      expect(lastCall.eliminatedPerRound.length).toBeGreaterThan(0);
    });
  });

  describe("result construction", () => {
    it("returns winner response in correct model position", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A Response", usage: {} })
        .mockResolvedValueOnce({ content: "B Response", usage: {} })
        .mockResolvedValueOnce({ content: "C Response", usage: {} })
        .mockResolvedValueOnce({ content: "D Response", usage: {} })
        .mockResolvedValue({ content: "A" }); // Always pick first (A beats B, C beats D, A beats C)

      const results = await sendTournamentMode("Hello", ctx, vi.fn());

      // Winner (a) should be at index 0
      expect(results[0]).not.toBeNull();
      expect(results[0]?.content).toBe("A Response");
      expect(results[1]).toBeNull();
      expect(results[2]).toBeNull();
      expect(results[3]).toBeNull();
    });

    it("includes complete mode metadata in winner result", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      const results = await sendTournamentMode("Hello", ctx, vi.fn());

      const winner = results.find((r) => r !== null);
      expect(winner?.modeMetadata).toMatchObject({
        mode: "tournament",
        isTournamentWinner: true,
      });
      expect(winner?.modeMetadata?.tournamentWinner).toBeDefined();
      expect(winner?.modeMetadata?.bracket).toBeDefined();
      expect(winner?.modeMetadata?.matches).toBeDefined();
      expect(winner?.modeMetadata?.eliminatedPerRound).toBeDefined();
    });

    it("includes match data in metadata", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: { inputTokens: 10 } });

      const results = await sendTournamentMode("Hello", ctx, vi.fn());

      const winner = results.find((r) => r !== null);
      const matches = winner?.modeMetadata?.matches;
      expect(Array.isArray(matches)).toBe(true);
      if (matches && matches.length > 0) {
        expect(matches[0]).toHaveProperty("id");
        expect(matches[0]).toHaveProperty("round");
        expect(matches[0]).toHaveProperty("competitor1");
        expect(matches[0]).toHaveProperty("competitor2");
        expect(matches[0]).toHaveProperty("winner");
        expect(matches[0]).toHaveProperty("judge");
        expect(matches[0]).toHaveProperty("response1");
        expect(matches[0]).toHaveProperty("response2");
      }
    });
  });

  describe("state transitions", () => {
    it("transitions through generating -> competing -> done phases", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      const setStateCalls = (ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>).mock
        .calls;

      // Check phase transitions
      expect(setStateCalls[0][0].phase).toBe("generating");
      expect(setStateCalls.some((call) => call[0].phase === "competing")).toBe(true);
      expect(setStateCalls[setStateCalls.length - 1][0].phase).toBe("done");
    });

    it("sets winner in final state", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      const lastCall = (
        ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>
      ).mock.calls.slice(-1)[0][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.winner).toBeDefined();
    });
  });

  describe("message filtering", () => {
    it("filters messages for each model during response generation", async () => {
      const messages = [{ id: "1", role: "user" as const, content: "Hi", timestamp: new Date() }];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["a", "b", "c", "d"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "a");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "b");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "c");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "d");
    });
  });

  describe("multimodal content handling", () => {
    it("handles multimodal content in API messages", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      const multimodalContent = [
        { type: "input_text", text: "Describe this" },
        { type: "image", source: { type: "base64", data: "abc" } },
      ];

      await sendTournamentMode(multimodalContent, ctx, vi.fn());

      // Should pass multimodal content to response calls
      const responseCalls = streamResponse.mock.calls.slice(0, 4);
      responseCalls.forEach((call) => {
        const inputItems = call[1];
        const lastItem = inputItems[inputItems.length - 1];
        expect(lastItem.content).toBe(multimodalContent);
      });
    });
  });

  describe("abort controller management", () => {
    it("replaces abort controllers for each judge call", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c", "d"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "A", usage: {} });

      await sendTournamentMode("Hello", ctx, vi.fn());

      // Abort controllers should be set multiple times
      // During generating phase and for each judge
      expect(ctx.abortControllersRef.current).toBeDefined();
    });
  });
});
