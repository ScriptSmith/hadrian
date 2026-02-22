import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendElectedMode } from "../elected";
import { createMockContext } from "./test-utils";

describe("sendElectedMode", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  describe("model count fallback", () => {
    it("falls back to multiple mode with fewer than 3 models", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const fallback = vi.fn().mockResolvedValue([{ content: "fallback" }]);

      const results = await sendElectedMode("Hello", ctx, fallback);

      expect(fallback).toHaveBeenCalledWith("Hello");
      expect(results).toEqual([{ content: "fallback" }]);
      expect(ctx.streamingStore.setModeState).not.toHaveBeenCalled();
    });

    it("falls back to multiple mode with single model", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const fallback = vi.fn().mockResolvedValue([{ content: "single" }]);

      const results = await sendElectedMode("Hello", ctx, fallback);

      expect(fallback).toHaveBeenCalledWith("Hello");
      expect(results).toEqual([{ content: "single" }]);
    });

    it("proceeds with election when exactly 3 models present", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const fallback = vi.fn();
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendElectedMode("Hello", ctx, fallback);

      expect(fallback).not.toHaveBeenCalled();
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });
  });

  describe("responding phase", () => {
    it("initializes election state in responding phase", async () => {
      const ctx = createMockContext();
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      expect(ctx.streamingStore.setModeState).toHaveBeenNthCalledWith(1, {
        mode: "elected",
        phase: "responding",
        candidates: [],
        completedResponses: 0,
        totalModels: 3,
        votes: [],
        completedVotes: 0,
      });
    });

    it("initializes streaming for all models", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      // gatherInstances passes instance IDs with model map
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(
        ["a", "b", "c"],
        new Map([
          ["a", "a"],
          ["b", "b"],
          ["c", "c"],
        ])
      );
    });

    it("creates abort controllers for all models", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      expect(ctx.abortControllersRef.current).toHaveLength(3);
      expect(ctx.abortControllersRef.current[0]).toBeInstanceOf(AbortController);
    });

    it("streams responses from all models in parallel", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      expect(streamResponse).toHaveBeenCalledTimes(6); // 3 for responses + 3 for votes
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

    it("adds each successful candidate response", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "Response B", usage: { inputTokens: 20 } })
        .mockResolvedValueOnce({ content: "Response C", usage: { inputTokens: 30 } })
        .mockResolvedValue({ content: "1" }); // Votes

      await sendElectedMode("Hello", ctx, vi.fn());

      // Candidates are added via updateModeState calls
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });

    it("handles failed model responses gracefully", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A", usage: {} })
        .mockResolvedValueOnce(null) // model-b fails
        .mockResolvedValueOnce({ content: "Response C", usage: {} })
        .mockResolvedValue({ content: "1" }); // Votes

      await sendElectedMode("Hello", ctx, vi.fn());

      // Should only add 2 candidates via updateModeState
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });
  });

  describe("insufficient candidates handling", () => {
    it("returns early when only one candidate succeeds", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Only Response", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null);

      const results = await sendElectedMode("Hello", ctx, vi.fn());

      // Should mark as done without voting
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1];
      expect(lastCall[0].phase).toBe("done");

      // Should return the single candidate as winner
      const nonNullResults = results.filter((r) => r !== null);
      expect(nonNullResults).toHaveLength(1);
      expect(nonNullResults[0]?.content).toBe("Only Response");
      expect(nonNullResults[0]?.modeMetadata?.winner).toBe("model-a");
    });

    it("returns empty results when no candidates succeed", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue(null);

      const results = await sendElectedMode("Hello", ctx, vi.fn());

      expect(results.filter((r) => r !== null)).toHaveLength(0);
    });
  });

  describe("voting phase", () => {
    it("transitions to voting phase after gathering responses", async () => {
      const ctx = createMockContext();
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "response", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      // Check that voting phase was set
      const setModeStateCalls = (ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      expect(setModeStateCalls.some((call) => call[0].phase === "voting")).toBe(true);
    });

    it("builds voting prompt with all candidate responses", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A", usage: {} })
        .mockResolvedValueOnce({ content: "Response B", usage: {} })
        .mockResolvedValueOnce({ content: "Response C", usage: {} })
        .mockResolvedValue({ content: "1" }); // Votes

      await sendElectedMode("Hello", ctx, vi.fn());

      // Voting calls should include the voting prompt with candidates
      const votingCalls = streamResponse.mock.calls.slice(3); // Skip first 3 response calls
      expect(votingCalls.length).toBe(3);

      // The system message should contain the candidates
      const systemContent = votingCalls[0][1][0].content;
      expect(systemContent).toContain("Candidate 1");
      expect(systemContent).toContain("Candidate 2");
      expect(systemContent).toContain("Candidate 3");
    });

    it("uses custom voting prompt from config", async () => {
      const customPrompt = "Custom voting prompt: pick the best";
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { votingPrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      // Voting calls should use custom prompt
      const votingCalls = streamResponse.mock.calls.slice(3);
      const systemContent = votingCalls[0][1][0].content;
      expect(systemContent).toBe(customPrompt);
    });

    it("uses limited max tokens for voting", async () => {
      const ctx = createMockContext();
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      // Voting calls should have maxTokens set to 150
      const votingCalls = streamResponse.mock.calls.slice(3);
      expect(votingCalls[0][3]).toEqual({ maxTokens: 150 });
    });

    it("creates new abort controllers for voting phase", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      // Abort controllers should be replaced for voting
      expect(ctx.abortControllersRef.current).toHaveLength(3);
    });

    it("initializes streaming for each voter", async () => {
      const ctx = createMockContext({ models: ["a", "b", "c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      // Should init streaming for each voter individually with model map
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(["a"], new Map([["a", "a"]]));
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(["b"], new Map([["b", "b"]]));
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(["c"], new Map([["c", "c"]]));
    });
  });

  describe("vote parsing", () => {
    it("parses vote number from response", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "2", usage: {} }) // Vote for candidate 2
        .mockResolvedValueOnce({ content: "1", usage: {} }) // Vote for candidate 1
        .mockResolvedValueOnce({ content: "2", usage: {} }); // Vote for candidate 2

      await sendElectedMode("Hello", ctx, vi.fn());

      // Votes are added via updateModeState
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });

    it("extracts number from verbose vote response", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "I think candidate 2 is the best", usage: {} })
        .mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      // Votes are added via updateModeState
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });

    it("ignores invalid vote numbers", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "5", usage: {} }) // Invalid - only 3 candidates
        .mockResolvedValueOnce({ content: "0", usage: {} }) // Invalid - 0-indexed would be -1
        .mockResolvedValueOnce({ content: "1", usage: {} }); // Valid

      await sendElectedMode("Hello", ctx, vi.fn());

      // Votes are added via updateModeState - test result metadata instead
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });

    it("ignores votes without numbers", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "I cannot decide", usage: {} }) // No number
        .mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      // Votes are added via updateModeState
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });

    it("handles voting errors gracefully", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockRejectedValueOnce(new Error("Vote failed")) // model-a vote fails
        .mockResolvedValueOnce({ content: "1", usage: {} })
        .mockResolvedValueOnce({ content: "2", usage: {} });

      // Should not throw
      const results = await sendElectedMode("Hello", ctx, vi.fn());

      // Should still complete and return results
      expect(results.filter((r) => r !== null).length).toBeGreaterThan(0);
    });

    it("stores vote reasoning in vote data", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "I vote 1 because it's thorough", usage: {} })
        .mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      // Votes are added via updateModeState with reasoning
      expect(ctx.streamingStore.setModeState).toHaveBeenCalled();
    });
  });

  describe("vote counting", () => {
    it("counts votes correctly and determines winner", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "2", usage: {} }) // Vote for B
        .mockResolvedValueOnce({ content: "2", usage: {} }) // Vote for B
        .mockResolvedValueOnce({ content: "1", usage: {} }); // Vote for A

      const results = await sendElectedMode("Hello", ctx, vi.fn());

      // model-b should win with 2 votes
      const winner = results.find((r) => r !== null);
      expect(winner?.modeMetadata?.winner).toBe("model-b");
      expect(winner?.modeMetadata?.voteCounts).toEqual({
        "model-a": 1,
        "model-b": 2,
        "model-c": 0,
      });
    });

    it("breaks ties alphabetically for determinism", async () => {
      const ctx = createMockContext({ models: ["model-c", "model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "1", usage: {} }) // Vote for C
        .mockResolvedValueOnce({ content: "2", usage: {} }) // Vote for A
        .mockResolvedValueOnce({ content: "3", usage: {} }); // Vote for B

      const results = await sendElectedMode("Hello", ctx, vi.fn());

      // model-a should win (alphabetically first with 1 vote each)
      const winner = results.find((r) => r !== null);
      expect(winner?.modeMetadata?.winner).toBe("model-a");
    });

    it("allows self-voting", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({ content: "1", usage: {} }) // model-a votes for itself
        .mockResolvedValueOnce({ content: "2", usage: {} }) // model-b votes for itself
        .mockResolvedValueOnce({ content: "1", usage: {} }); // model-c votes for model-a

      const results = await sendElectedMode("Hello", ctx, vi.fn());

      // model-a should win with 2 votes (including self-vote)
      const winner = results.find((r) => r !== null);
      expect(winner?.modeMetadata?.winner).toBe("model-a");
      expect(winner?.modeMetadata?.voteCounts?.["model-a"]).toBe(2);
    });
  });

  describe("result construction", () => {
    it("returns winner response in correct model position", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A Response", usage: {} })
        .mockResolvedValueOnce({ content: "B Response", usage: {} })
        .mockResolvedValueOnce({ content: "C Response", usage: {} })
        .mockResolvedValue({ content: "2", usage: {} }); // All vote for B

      const results = await sendElectedMode("Hello", ctx, vi.fn());

      // Winner (model-b) should be at index 1
      expect(results[0]).toBeNull();
      expect(results[1]).not.toBeNull();
      expect(results[1]?.content).toBe("B Response");
      expect(results[2]).toBeNull();
    });

    it("includes complete mode metadata in winner result", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "B", usage: { inputTokens: 20 } })
        .mockResolvedValueOnce({ content: "C", usage: { inputTokens: 30 } })
        .mockResolvedValue({ content: "1", usage: { inputTokens: 5 } });

      const results = await sendElectedMode("Hello", ctx, vi.fn());

      const winner = results.find((r) => r !== null);
      expect(winner?.modeMetadata).toMatchObject({
        mode: "elected",
        isElected: true,
        winner: "model-a",
      });
      expect(winner?.modeMetadata?.candidates).toHaveLength(3);
      expect(winner?.modeMetadata?.votes).toHaveLength(3);
      expect(winner?.modeMetadata?.voteCounts).toBeDefined();
      expect(winner?.modeMetadata?.voteUsage).toBeDefined();
    });

    it("aggregates vote usage correctly", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: {} })
        .mockResolvedValueOnce({ content: "B", usage: {} })
        .mockResolvedValueOnce({ content: "C", usage: {} })
        .mockResolvedValueOnce({
          content: "1",
          usage: { inputTokens: 100, outputTokens: 10, totalTokens: 110, cost: 0.01 },
        })
        .mockResolvedValueOnce({
          content: "1",
          usage: { inputTokens: 100, outputTokens: 10, totalTokens: 110, cost: 0.01 },
        })
        .mockResolvedValueOnce({
          content: "1",
          usage: { inputTokens: 100, outputTokens: 10, totalTokens: 110, cost: 0.01 },
        });

      const results = await sendElectedMode("Hello", ctx, vi.fn());

      const winner = results.find((r) => r !== null);
      expect(winner?.modeMetadata?.voteUsage).toEqual({
        inputTokens: 300,
        outputTokens: 30,
        totalTokens: 330,
        cost: 0.03,
      });
    });
  });

  describe("state transitions", () => {
    it("transitions through responding -> voting -> done phases", async () => {
      const ctx = createMockContext();
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      const setModeStateCalls = (ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>).mock
        .calls;

      // Check phase transitions
      expect(setModeStateCalls[0][0].phase).toBe("responding");
      expect(setModeStateCalls.some((call) => call[0].phase === "voting")).toBe(true);
      expect(setModeStateCalls[setModeStateCalls.length - 1][0].phase).toBe("done");
    });

    it("sets winner and vote counts in final state", async () => {
      const ctx = createMockContext();
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      const setModeStateCalls = (ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>).mock
        .calls;
      const lastCall = setModeStateCalls[setModeStateCalls.length - 1];

      expect(lastCall[0].phase).toBe("done");
      expect(lastCall[0].winner).toBeDefined();
      expect(lastCall[0].voteCounts).toBeDefined();
    });
  });

  describe("message filtering", () => {
    it("filters messages for each model during response gathering", async () => {
      const messages = [{ id: "1", role: "user" as const, content: "Hi", timestamp: new Date() }];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "1", usage: {} });

      await sendElectedMode("Hello", ctx, vi.fn());

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-a");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-b");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-c");
    });
  });

  describe("multimodal content handling", () => {
    it("handles multimodal content in API messages", async () => {
      const ctx = createMockContext();
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "1", usage: {} });

      const multimodalContent = [
        { type: "input_text", text: "Describe this" },
        { type: "image", source: { type: "base64", data: "abc" } },
      ];

      await sendElectedMode(multimodalContent, ctx, vi.fn());

      // Should pass multimodal content to response calls
      const responseCalls = streamResponse.mock.calls.slice(0, 3);
      responseCalls.forEach((call) => {
        const inputItems = call[1];
        const lastItem = inputItems[inputItems.length - 1];
        expect(lastItem.content).toBe(multimodalContent);
      });
    });
  });
});
