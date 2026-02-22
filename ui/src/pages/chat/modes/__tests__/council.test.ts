import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendCouncilMode } from "../council";
import { createMockContext, createMockFallback } from "./test-utils";

describe("sendCouncilMode", () => {
  let mockFallback: ReturnType<typeof createMockFallback>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockFallback = createMockFallback();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode with single model", async () => {
      const ctx = createMockContext({ models: ["model-a"] });

      await sendCouncilMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
      expect(ctx.streamResponse).not.toHaveBeenCalled();
    });

    it("does not fall back with two models (1 synthesizer + 1 council member)", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response", usage: {} });

      await sendCouncilMode("Hello", ctx, mockFallback);

      expect(mockFallback).not.toHaveBeenCalled();
      expect(streamResponse).toHaveBeenCalled();
    });

    it("falls back when all models are the synthesizer (no council members)", async () => {
      // If synthesizerModel is the only model, no council members remain
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { synthesizerModel: "model-a" },
      });

      await sendCouncilMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
    });
  });

  describe("role assignment", () => {
    it("assigns default roles to council members (excluding synthesizer)", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Question", ctx, mockFallback);

      // First call should have roles (opening phase)
      // model-a is synthesizer, model-b and model-c are council members
      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      expect(openingCall).toBeDefined();
      const roles = openingCall[0].roles;
      expect(roles["model-b"]).toBe("Technical Expert");
      expect(roles["model-c"]).toBe("Business Analyst");
      expect(roles["model-a"]).toBeUndefined(); // Synthesizer not in roles
    });

    it("uses configured roles when provided", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: {
          councilRoles: {
            "model-b": "Custom Role A",
            "model-c": "Custom Role B",
          },
        },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Question", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      const roles = openingCall[0].roles;
      expect(roles["model-b"]).toBe("Custom Role A");
      expect(roles["model-c"]).toBe("Custom Role B");
    });

    it("cycles through default roles for many council members", async () => {
      const ctx = createMockContext({
        models: [
          "synth",
          "member-1",
          "member-2",
          "member-3",
          "member-4",
          "member-5",
          "member-6",
          "member-7",
          "member-8",
          "member-9",
        ],
        modeConfig: { synthesizerModel: "synth" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Question", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      const roles = openingCall[0].roles;
      // 9th member should cycle back to first default role
      expect(roles["member-9"]).toBe("Technical Expert");
    });
  });

  describe("auto-assign roles", () => {
    it("starts with assigning phase when autoAssignRoles is true", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { councilAutoAssignRoles: true },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"model-b": "Analyst", "model-c": "Engineer"}',
      });

      await sendCouncilMode("Question", ctx, mockFallback);

      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.mode).toBe("council");
      expect(firstCall.phase).toBe("assigning");
      expect(firstCall.currentRound).toBe(0);
      expect(firstCall.totalRounds).toBe(2);
      expect(firstCall.roles).toEqual({});
      expect(firstCall.synthesizerModel).toBe("model-a");
    });

    it("uses synthesizer model to assign roles", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { councilAutoAssignRoles: true, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"model-b": "Analyst", "model-c": "Engineer"}',
      });

      await sendCouncilMode("Question", ctx, mockFallback);

      // First call should be role assignment to synthesizer
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
    });

    it("parses assigned roles from JSON response", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { councilAutoAssignRoles: true },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"model-b": "Security Expert", "model-c": "UX Designer"}',
      });
      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Question", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      const roles = openingCall[0].roles;
      expect(roles["model-b"]).toBe("Security Expert");
      expect(roles["model-c"]).toBe("UX Designer");
    });

    it("falls back to default roles when JSON parsing fails", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { councilAutoAssignRoles: true },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Invalid JSON response",
      });
      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Question", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      const roles = openingCall[0].roles;
      expect(roles["model-b"]).toBe("Technical Expert");
      expect(roles["model-c"]).toBe("Business Analyst");
    });

    it("falls back to default roles when role assignment fails", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { councilAutoAssignRoles: true },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockRejectedValueOnce(new Error("Network error"))
        .mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Question", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      const roles = openingCall[0].roles;
      expect(roles["model-b"]).toBe("Technical Expert");
    });

    it("falls back when role assignment returns null", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { councilAutoAssignRoles: true },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce(null).mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Question", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      const roles = openingCall[0].roles;
      expect(roles["model-b"]).toBe("Technical Expert");
    });

    it("tracks role assignment usage", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { councilAutoAssignRoles: true },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"model-b": "Expert", "model-c": "Analyst"}',
        usage: { inputTokens: 50, outputTokens: 25 },
      });
      streamResponse.mockResolvedValue({
        content: "Response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      const results = await sendCouncilMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      // Should include role assignment usage in aggregate
      expect(result?.modeMetadata?.aggregateUsage?.inputTokens).toBeGreaterThan(50);
    });
  });

  describe("opening phase", () => {
    it("initializes council state with opening phase after role assignment", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Opening perspective" });

      await sendCouncilMode("Council topic", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      expect(openingCall).toBeDefined();
      expect(openingCall[0].mode).toBe("council");
      expect(openingCall[0].currentRound).toBe(0);
      expect(openingCall[0].totalRounds).toBe(2);
      expect(openingCall[0].synthesizerModel).toBe("model-a");
    });

    it("uses custom discussion rounds from config", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 4 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      expect(openingCall[0].totalRounds).toBe(4);
    });

    it("uses custom synthesizer model from config", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-c" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      expect(openingCall[0].synthesizerModel).toBe("model-c");
    });

    it("initializes streaming for council members only (not synthesizer)", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // First initStreaming for opening should be council members only
      // Instance-aware streaming passes [instanceIds], modelMap
      expect(initStreaming).toHaveBeenCalledWith(
        ["model-b", "model-c"],
        new Map([
          ["model-b", "model-b"],
          ["model-c", "model-c"],
        ])
      );
    });

    it("gathers opening perspectives from all council members in parallel", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Opening perspective" });

      await sendCouncilMode("What should we build?", ctx, mockFallback);

      // Should call streamResponse for council members (model-b and model-c)
      // Instance-aware streaming: (modelId, inputItems, controller, settings, instanceId, trackToolCalls, onSSEEvent, instanceParams, instanceLabel)
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

    it("includes role in opening prompt", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Design question", ctx, mockFallback);

      // First council member call (model-b = Technical Expert)
      const firstCall = streamResponse.mock.calls[0];
      const inputItems = firstCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("Technical Expert");
      expect(systemPrompt).toContain("Design question");
    });

    it("uses custom council prompt from config", async () => {
      const customPrompt = "Custom council prompt for {role}";
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { councilPrompt: customPrompt, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      const firstCall = streamResponse.mock.calls[0];
      const inputItems = firstCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toBe(customPrompt);
    });

    it("adds opening statements via updateModeState", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Technical perspective", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "Business perspective", usage: { inputTokens: 15 } })
        .mockResolvedValue({ content: "Synthesis" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // updateModeState is called with an updater function to add statements
      expect(updateModeState).toHaveBeenCalled();

      // Simulate the updater to verify it adds the correct statement
      const mockState = {
        mode: "council" as const,
        phase: "opening" as const,
        currentRound: 0,
        totalRounds: 2,
        roles: { "model-b": "Technical Expert", "model-c": "Business Analyst" },
        statements: [] as Array<{
          model: string;
          role: string;
          content: string;
          round: number;
        }>,
        currentRoundStatements: [] as Array<{
          model: string;
          role: string;
          content: string;
          round: number;
        }>,
        synthesizerModel: "model-a",
      };

      const firstUpdater = updateModeState.mock.calls[0][0];
      const afterFirst = firstUpdater(mockState);
      expect(afterFirst.statements).toHaveLength(1);
      expect(afterFirst.statements[0].content).toBe("Technical perspective");
      expect(afterFirst.statements[0].round).toBe(0);
    });
  });

  describe("insufficient opening responses", () => {
    it("returns early when no opening responses succeed", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue(null);

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      // Should mark as done immediately
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");

      // Should return all nulls
      expect(results).toEqual([null, null, null]);
    });

    it("continues with partial opening responses", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Only one perspective" })
        .mockResolvedValueOnce(null)
        .mockResolvedValue({ content: "Synthesis" });

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      // Should complete with synthesis
      const result = results.find((r) => r !== null);
      expect(result).not.toBeNull();
    });
  });

  describe("discussion rounds", () => {
    it("transitions to discussing phase for discussion rounds", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      const discussingCall = setModeState.mock.calls.find(
        (call) => call[0].phase === "discussing"
      )!;
      expect(discussingCall).toBeDefined();
      expect(discussingCall[0].currentRound).toBe(1);
    });

    it("re-initializes streaming for each discussion round", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 2, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // Opening + 2 discussion rounds + synthesis = 4 calls
      expect(initStreaming).toHaveBeenCalledTimes(4);
    });

    it("creates new abort controllers for each discussion round", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // Verify abort controllers were updated
      expect(ctx.abortControllersRef.current).toBeDefined();
    });

    it("includes previous round perspectives in discussion prompt", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // Opening responses
      streamResponse
        .mockResolvedValueOnce({ content: "Technical perspective content" })
        .mockResolvedValueOnce({ content: "Business perspective content" })
        // Discussion responses
        .mockResolvedValueOnce({ content: "Technical response" })
        .mockResolvedValueOnce({ content: "Business response" })
        // Synthesis
        .mockResolvedValueOnce({ content: "Synthesis" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // Check discussion calls (3rd and 4th calls)
      const discussionCall = streamResponse.mock.calls[2];
      const inputItems = discussionCall[1];
      const systemPrompt = inputItems[0].content;
      expect(systemPrompt).toContain("Technical perspective content");
      expect(systemPrompt).toContain("Business perspective content");
    });

    it("adds discussion statements to correct round via updateModeState", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        .mockResolvedValueOnce({ content: "Discussion A", usage: { inputTokens: 20 } })
        .mockResolvedValueOnce({ content: "Discussion B", usage: { inputTokens: 25 } })
        .mockResolvedValueOnce({ content: "Synthesis" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // updateModeState should have been called for discussion statements
      // 2 opening + 2 discussion = 4 statements
      expect(updateModeState.mock.calls.length).toBeGreaterThanOrEqual(4);

      // Verify discussion statements have round 1
      const mockState = {
        mode: "council" as const,
        phase: "discussing" as const,
        currentRound: 1,
        totalRounds: 1,
        roles: { "model-b": "Technical Expert", "model-c": "Business Analyst" },
        statements: [
          { model: "model-b", role: "Technical Expert", content: "Opening A", round: 0 },
          { model: "model-c", role: "Business Analyst", content: "Opening B", round: 0 },
        ],
        currentRoundStatements: [] as Array<{
          model: string;
          role: string;
          content: string;
          round: number;
        }>,
        synthesizerModel: "model-a",
      };

      // The 3rd call should add the first discussion statement
      const thirdUpdater = updateModeState.mock.calls[2][0];
      const afterDiscussion = thirdUpdater(mockState);
      expect(afterDiscussion.statements).toHaveLength(3);
      expect(afterDiscussion.statements[2].content).toBe("Discussion A");
      expect(afterDiscussion.statements[2].round).toBe(1);
    });

    it("handles discussion failures gracefully", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        // Discussions return null (failed)
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce(null)
        // Synthesis still runs
        .mockResolvedValueOnce({ content: "Synthesis" });

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      // Should still complete with synthesis
      const result = results.find((r) => r !== null);
      expect(result?.content).toBe("Synthesis");
    });
  });

  describe("synthesizing phase", () => {
    it("transitions to synthesizing phase after discussions", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      const synthesizingCall = setModeState.mock.calls.find(
        (call) => call[0].phase === "synthesizing"
      )!;
      expect(synthesizingCall).toBeDefined();
      expect(synthesizingCall[0].currentRound).toBe(1);
    });

    it("initializes streaming for synthesizer model only", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 0, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // Last init should be for synthesizer only
      // Instance-aware streaming passes [instanceIds], modelMap
      const lastInitCall = initStreaming.mock.calls[initStreaming.mock.calls.length - 1];
      expect(lastInitCall[0]).toEqual(["model-a"]);
      expect(lastInitCall[1]).toEqual(new Map([["model-a", "model-a"]]));
    });

    it("includes full council transcript in synthesis prompt", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening tech" })
        .mockResolvedValueOnce({ content: "Opening business" })
        .mockResolvedValueOnce({ content: "Discussion tech" })
        .mockResolvedValueOnce({ content: "Discussion business" })
        .mockResolvedValueOnce({ content: "Final synthesis" });

      await sendCouncilMode("Council question", ctx, mockFallback);

      // Check synthesis call (last call)
      const synthesisCall = streamResponse.mock.calls[4];
      const inputItems = synthesisCall[1];
      const systemPrompt = inputItems[0].content;
      expect(systemPrompt).toContain("Council question");
      expect(systemPrompt).toContain("Opening tech");
      expect(systemPrompt).toContain("Opening business");
      expect(systemPrompt).toContain("Discussion tech");
      expect(systemPrompt).toContain("Discussion business");
    });

    it("handles synthesis failure with fallback message", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 0, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        .mockRejectedValueOnce(new Error("Synthesis failed"));

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.content).toContain("could not synthesize");
    });
  });

  describe("result construction", () => {
    it("returns synthesis as the result", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 0, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        .mockResolvedValueOnce({ content: "Final synthesis content" });

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.content).toBe("Final synthesis content");
    });

    it("places result at synthesizer model index", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-b", debateRounds: 0 },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      // Result should be at index 1 (model-b)
      expect(results[0]).toBeNull();
      expect(results[1]).not.toBeNull();
      expect(results[2]).toBeNull();
    });

    it("includes mode metadata with council info", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata).toBeDefined();
      expect(result?.modeMetadata?.mode).toBe("council");
      expect(result?.modeMetadata?.isCouncilSynthesis).toBe(true);
      expect(result?.modeMetadata?.councilRoles).toBeDefined();
      expect(result?.modeMetadata?.councilStatements).toBeDefined();
      expect(result?.modeMetadata?.councilRounds).toBeDefined();
      expect(result?.modeMetadata?.summarizerModel).toBe("model-a");
    });

    it("includes all council statements in metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Opening A" })
        .mockResolvedValueOnce({ content: "Opening B" })
        .mockResolvedValueOnce({ content: "Discussion A" })
        .mockResolvedValueOnce({ content: "Discussion B" })
        .mockResolvedValueOnce({ content: "Synthesis" });

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata?.councilStatements).toHaveLength(4);
    });

    it("aggregates usage from all statements and synthesis", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 0, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: { inputTokens: 10, outputTokens: 20 } })
        .mockResolvedValueOnce({ content: "B", usage: { inputTokens: 15, outputTokens: 25 } })
        .mockResolvedValueOnce({ content: "S", usage: { inputTokens: 30, outputTokens: 40 } });

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      // Total: 10+15+30 = 55 input, 20+25+40 = 85 output
      expect(result?.modeMetadata?.aggregateUsage?.inputTokens).toBe(55);
      expect(result?.modeMetadata?.aggregateUsage?.outputTokens).toBe(85);
    });

    it("includes synthesis usage separately in metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 0, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A", usage: { inputTokens: 10 } })
        .mockResolvedValueOnce({ content: "B", usage: { inputTokens: 15 } })
        .mockResolvedValueOnce({
          content: "Synthesis",
          usage: { inputTokens: 50, outputTokens: 100 },
        });

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata?.summaryUsage).toEqual({ inputTokens: 50, outputTokens: 100 });
    });

    it("calculates councilRounds including opening round", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 2, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendCouncilMode("Topic", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      // Opening (1) + discussion rounds (2) = 3
      expect(result?.modeMetadata?.councilRounds).toBe(3);
    });
  });

  describe("state transitions", () => {
    it("transitions through opening -> discussing -> synthesizing -> done phases", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 1, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      const phases = setModeState.mock.calls.map((call) => call[0].phase);
      expect(phases[0]).toBe("opening");
      expect(phases).toContain("discussing");
      expect(phases).toContain("synthesizing");
      expect(phases[phases.length - 1]).toBe("done");
    });

    it("skips discussing phase when debateRounds is 0", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 0, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      const phases = setModeState.mock.calls.map((call) => call[0].phase);
      expect(phases).not.toContain("discussing");
      expect(phases).toEqual(["opening", "synthesizing", "done"]);
    });

    it("sets final state with synthesis content and usage", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 0, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "A" })
        .mockResolvedValueOnce({ content: "B" })
        .mockResolvedValueOnce({ content: "Final synthesis", usage: { inputTokens: 50 } });

      await sendCouncilMode("Topic", ctx, mockFallback);

      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.synthesis).toBe("Final synthesis");
      expect(lastCall.synthesisUsage).toEqual({ inputTokens: 50 });
    });
  });

  describe("abort controller management", () => {
    it("creates abort controllers for council members in opening", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // Council members (model-b and model-c) should have controllers
      // Instance-aware streaming: (modelId, inputItems, controller, settings, instanceId, trackToolCalls, onSSEEvent, instanceParams, instanceLabel)
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
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // After completion, ref should have the synthesis controller
      expect(ctx.abortControllersRef.current).toBeDefined();
      expect(ctx.abortControllersRef.current.length).toBeGreaterThanOrEqual(1);
    });

    it("creates separate abort controller for synthesis phase", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 0, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // Synthesis uses single abort controller
      expect(ctx.abortControllersRef.current).toHaveLength(1);
    });
  });

  describe("message filtering", () => {
    it("filters messages for each council member in opening round", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Hello", timestamp: new Date() },
        {
          id: "2",
          role: "assistant" as const,
          content: "Hi",
          model: "model-b",
          timestamp: new Date(),
        },
      ];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        messages,
        filterMessagesForModel,
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-b");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-c");
    });
  });

  describe("multimodal content handling", () => {
    it("extracts text from multimodal content for prompts", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const multimodalContent = [
        { type: "input_text", text: "Discuss this image" },
        { type: "image", source: { type: "base64", data: "abc123" } },
      ];

      await sendCouncilMode(multimodalContent, ctx, mockFallback);

      // Check opening prompt contains extracted text
      const firstCall = streamResponse.mock.calls[0];
      const inputItems = firstCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("Discuss this image");
    });

    it("passes multimodal content to fallback when needed", async () => {
      const ctx = createMockContext({ models: ["model-a"] });

      const multimodalContent = [
        { type: "input_text", text: "Council topic" },
        { type: "image", source: { type: "base64", data: "xyz" } },
      ];

      await sendCouncilMode(multimodalContent, ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith(multimodalContent);
    });
  });

  describe("multiple discussion rounds", () => {
    it("runs correct number of discussion rounds", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 3, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // Count discussing phase calls
      const discussingCalls = setModeState.mock.calls.filter(
        (call) => call[0].phase === "discussing"
      );
      expect(discussingCalls).toHaveLength(3);

      // Verify round numbers
      expect(discussingCalls[0][0].currentRound).toBe(1);
      expect(discussingCalls[1][0].currentRound).toBe(2);
      expect(discussingCalls[2][0].currentRound).toBe(3);
    });

    it("accumulates statements across all rounds", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { debateRounds: 2, synthesizerModel: "model-a" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // Opening (2 members) + Round 1 (2) + Round 2 (2) = 6 statements
      expect(updateModeState).toHaveBeenCalledTimes(6);
    });
  });

  describe("synthesizer exclusion from council", () => {
    it("excludes synthesizer from council members", async () => {
      const ctx = createMockContext({
        models: ["synth", "member-a", "member-b"],
        modeConfig: { synthesizerModel: "synth" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      // Synthesizer should only be called for synthesis, not opening
      const synthCalls = streamResponse.mock.calls.filter((call) => call[0] === "synth");
      expect(synthCalls).toHaveLength(1); // Only the synthesis call
    });

    it("synthesizer does not have a role assigned", async () => {
      const ctx = createMockContext({
        models: ["synth", "member-a", "member-b"],
        modeConfig: { synthesizerModel: "synth" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendCouncilMode("Topic", ctx, mockFallback);

      const openingCall = setModeState.mock.calls.find((call) => call[0].phase === "opening")!;
      const roles = openingCall[0].roles;
      expect(roles["synth"]).toBeUndefined();
      expect(roles["member-a"]).toBe("Technical Expert");
      expect(roles["member-b"]).toBe("Business Analyst");
    });
  });
});
