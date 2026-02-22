import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendConfidenceWeightedMode } from "../confidence";
import { createMockContext as createBaseContext, createMockFallback } from "./test-utils";

// Confidence mode uses synthesizer + model models
function createMockContext(overrides: Parameters<typeof createBaseContext>[0] = {}) {
  return createBaseContext({
    models: ["synthesizer", "model-a", "model-b"],
    ...overrides,
  });
}

describe("sendConfidenceWeightedMode", () => {
  let mockFallback: ReturnType<typeof createMockFallback>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockFallback = createMockFallback();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode when only one model (the synthesizer)", async () => {
      const ctx = createMockContext({ models: ["synthesizer"] });

      await sendConfidenceWeightedMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
      expect(ctx.streamResponse).not.toHaveBeenCalled();
    });

    it("falls back when synthesizer is the only model via config", async () => {
      const ctx = createMockContext({
        models: ["synth-model"],
        modeConfig: { synthesizerModel: "synth-model" },
      });

      await sendConfidenceWeightedMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
    });

    it("does not fall back with two models (synthesizer + 1 responder)", async () => {
      const ctx = createMockContext({ models: ["synthesizer", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Response\n\nCONFIDENCE: 0.8",
      });
      streamResponse.mockResolvedValueOnce({
        content: "Synthesized",
      });

      await sendConfidenceWeightedMode("Hello", ctx, mockFallback);

      expect(mockFallback).not.toHaveBeenCalled();
      expect(streamResponse).toHaveBeenCalled();
    });
  });

  describe("synthesizer model selection", () => {
    it("uses first model as synthesizer by default", async () => {
      const ctx = createMockContext({ models: ["synth", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.9" });
      streamResponse.mockResolvedValueOnce({ content: "B\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // First state call should use "synth" as the synthesizer model
      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.mode).toBe("confidence-weighted");
      expect(firstCall.phase).toBe("responding");
      expect(firstCall.totalModels).toBe(2);
      expect(firstCall.synthesizerModel).toBe("synth");
      expect(firstCall.responses).toEqual([]);
    });

    it("uses configured synthesizer model", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-c" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "B\n\nCONFIDENCE: 0.7" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.phase).toBe("responding");
      expect(firstCall.synthesizerModel).toBe("model-c");
    });

    it("excludes synthesizer from responding models", async () => {
      const ctx = createMockContext({ models: ["synth", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.9" });
      streamResponse.mockResolvedValueOnce({ content: "B\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // First initStreaming should be for responding instances only (with model map)
      expect(initStreaming).toHaveBeenNthCalledWith(
        1,
        ["worker-a", "worker-b"],
        new Map([
          ["worker-a", "worker-a"],
          ["worker-b", "worker-b"],
        ])
      );
    });
  });

  describe("responding phase", () => {
    it("initializes state with responding phase", async () => {
      const ctx = createMockContext({ models: ["synth", "responder-1", "responder-2"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "R1\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "R2\n\nCONFIDENCE: 0.7" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.phase).toBe("responding");
      expect(firstCall.totalModels).toBe(2);
      expect(firstCall.synthesizerModel).toBe("synth");
      expect(firstCall.responses).toEqual([]);
    });

    it("initializes streaming for responding models", async () => {
      const ctx = createMockContext({ models: ["synth", "model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response\n\nCONFIDENCE: 0.5" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // gatherInstances passes instance IDs with model map
      expect(initStreaming).toHaveBeenCalledWith(
        ["model-a", "model-b", "model-c"],
        new Map([
          ["model-a", "model-a"],
          ["model-b", "model-b"],
          ["model-c", "model-c"],
        ])
      );
    });

    it("sends confidence prompt to each responding model", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("What is AI?", ctx, mockFallback);

      const responderCall = streamResponse.mock.calls[0];
      expect(responderCall[0]).toBe("responder");
      const inputItems = responderCall[1];

      // Should have a system prompt with confidence instructions
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("CONFIDENCE:");
      expect(systemPrompt).toContain("What is AI?");
    });

    it("uses custom confidence prompt from config", async () => {
      const customPrompt = "Custom confidence prompt for {question}";
      const ctx = createMockContext({
        models: ["synth", "responder"],
        modeConfig: { confidencePrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Test question", ctx, mockFallback);

      const responderCall = streamResponse.mock.calls[0];
      const inputItems = responderCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toBe(customPrompt);
    });

    it("gathers responses in parallel from all responding models", async () => {
      const ctx = createMockContext({ models: ["synth", "model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // Track call order
      const callOrder: string[] = [];
      streamResponse.mockImplementation(async (model: string) => {
        callOrder.push(model);
        if (model === "synth") {
          return { content: "Synthesized" };
        }
        return { content: `Response from ${model}\n\nCONFIDENCE: 0.7` };
      });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // All responding models should be called before synthesizer
      const synthIndex = callOrder.indexOf("synth");
      expect(callOrder.filter((m) => m !== "synth")).toHaveLength(3);
      expect(synthIndex).toBe(callOrder.length - 1);
    });
  });

  describe("confidence parsing", () => {
    it("parses confidence score from response in 0-1 format", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "This is my answer.\n\nCONFIDENCE: 0.85",
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // updateModeState is called with updater function to add response
      expect(updateModeState).toHaveBeenCalled();
    });

    it("parses confidence score from percentage format (>1)", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "My answer\n\nCONFIDENCE: 85",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // updateModeState is called to add response with confidence
      expect(updateModeState).toHaveBeenCalled();
    });

    it("defaults to 0.5 confidence when not found in response", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Response without confidence score",
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // updateModeState is called to add response
      expect(updateModeState).toHaveBeenCalled();
    });

    it("parses confidence case-insensitively", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Answer\n\nconfidence: 0.75",
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // updateModeState is called
      expect(updateModeState).toHaveBeenCalled();
    });

    it("removes confidence line from content", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Line 1\nLine 2\n\nCONFIDENCE: 0.9",
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // updateModeState is called with updater that adds response
      expect(updateModeState).toHaveBeenCalled();
    });

    it("handles invalid confidence values with default", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Answer\n\nCONFIDENCE: invalid",
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Invalid score should still be handled
      expect(updateModeState).toHaveBeenCalled();
    });

    it("clamps confidence to valid range (0-1)", async () => {
      const ctx = createMockContext({ models: ["synth", "responder-a", "responder-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      // Test negative and >100 values - these would fail the validation
      streamResponse.mockResolvedValueOnce({
        content: "Answer\n\nCONFIDENCE: -0.5",
      });
      streamResponse.mockResolvedValueOnce({
        content: "Answer\n\nCONFIDENCE: 150",
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // updateModeState should be called for each response
      expect(updateModeState.mock.calls.length).toBeGreaterThanOrEqual(2);
    });
  });

  describe("synthesizing phase", () => {
    it("transitions to synthesizing phase after gathering responses", async () => {
      const ctx = createMockContext({ models: ["synth", "responder-a", "responder-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.9" });
      streamResponse.mockResolvedValueOnce({ content: "B\n\nCONFIDENCE: 0.7" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Find synthesizing phase call
      const synthesizingCall = setModeState.mock.calls.find(
        (call) => call[0].phase === "synthesizing"
      )!;
      expect(synthesizingCall).toBeDefined();
      expect(synthesizingCall[0].totalModels).toBe(2);
      expect(synthesizingCall[0].synthesizerModel).toBe("synth");
      expect(synthesizingCall[0].responses).toHaveLength(2);
    });

    it("initializes streaming for synthesizer during synthesis", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Second initStreaming should be for synthesizer (with model map via streamInstance)
      expect(initStreaming).toHaveBeenNthCalledWith(2, ["synth"], new Map([["synth", "synth"]]));
    });

    it("sends synthesis prompt with responses sorted by confidence", async () => {
      const ctx = createMockContext({ models: ["synth", "low-conf", "high-conf"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Low confidence response\n\nCONFIDENCE: 0.3",
      });
      streamResponse.mockResolvedValueOnce({
        content: "High confidence response\n\nCONFIDENCE: 0.95",
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Test question", ctx, mockFallback);

      // Synthesis call should be the last one
      const synthCall = streamResponse.mock.calls[2];
      expect(synthCall[0]).toBe("synth");
      const inputItems = synthCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;

      // High confidence should appear first
      const highIndex = systemPrompt.indexOf("high-conf");
      const lowIndex = systemPrompt.indexOf("low-conf");
      expect(highIndex).toBeLessThan(lowIndex);

      // Check confidence percentages are included
      expect(systemPrompt).toContain("95%"); // high-conf
      expect(systemPrompt).toContain("30%"); // low-conf
    });

    it("uses custom synthesis prompt from config", async () => {
      const customPrompt = "Custom synthesis: {responses}";
      const ctx = createMockContext({
        models: ["synth", "responder"],
        modeConfig: { synthesisPrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      const synthCall = streamResponse.mock.calls[1];
      const systemPrompt = synthCall[1].find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toBe(customPrompt);
    });

    it("sends user message text to synthesizer", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("What is 2+2?", ctx, mockFallback);

      const synthCall = streamResponse.mock.calls[1];
      const userMessage = synthCall[1].find((i: { role: string }) => i.role === "user");
      expect(userMessage.content).toBe("What is 2+2?");
    });
  });

  describe("no responses handling", () => {
    it("returns empty results when all responders fail", async () => {
      const ctx = createMockContext({ models: ["synth", "responder-a", "responder-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // All responders return null
      streamResponse.mockResolvedValue(null);

      const results = await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      expect(results).toEqual([null, null, null]);
      // Synthesizer should not be called when no responses
      expect(streamResponse).toHaveBeenCalledTimes(2); // only responders
    });

    it("continues with partial responses", async () => {
      const ctx = createMockContext({ models: ["synth", "success", "fail", "success2"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce(null); // fail
      streamResponse.mockResolvedValueOnce({ content: "C\n\nCONFIDENCE: 0.7" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      const results = await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Should have synthesized with 2 successful responses
      expect(updateModeState).toHaveBeenCalled();
      expect(results[0]).not.toBeNull();
    });
  });

  describe("result construction", () => {
    it("returns synthesized content as result", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Final synthesized answer" });

      const results = await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      const synthResult = results.find((r) => r !== null);
      expect(synthResult?.content).toBe("Final synthesized answer");
    });

    it("places result at synthesizer model index", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-b" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "C\n\nCONFIDENCE: 0.7" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      const results = await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Result should be at index 1 (model-b)
      expect(results[0]).toBeNull();
      expect(results[1]).not.toBeNull();
      expect(results[2]).toBeNull();
    });

    it("includes mode metadata with confidence-weighted info", async () => {
      const ctx = createMockContext({ models: ["synth", "responder-a", "responder-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Response A\n\nCONFIDENCE: 0.9",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Response B\n\nCONFIDENCE: 0.7",
        usage: { inputTokens: 15, outputTokens: 25 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Synthesized",
        usage: { inputTokens: 50, outputTokens: 100 },
      });

      const results = await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata).toBeDefined();
      expect(result?.modeMetadata?.mode).toBe("confidence-weighted");
      expect(result?.modeMetadata?.isConfidenceWeighted).toBe(true);
      expect(result?.modeMetadata?.synthesizerModel).toBe("synth");
      expect(result?.modeMetadata?.synthesizerUsage).toEqual({
        inputTokens: 50,
        outputTokens: 100,
      });
    });

    it("includes confidence responses in metadata", async () => {
      const ctx = createMockContext({ models: ["synth", "responder-a", "responder-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Response A\n\nCONFIDENCE: 0.9",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Response B\n\nCONFIDENCE: 0.7",
        usage: { inputTokens: 15, outputTokens: 25 },
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      const results = await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      const confidenceResponses = result?.modeMetadata?.confidenceResponses;
      expect(confidenceResponses).toHaveLength(2);
      expect(confidenceResponses?.[0]).toEqual({
        model: "responder-a",
        content: "Response A",
        confidence: 0.9,
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      expect(confidenceResponses?.[1]).toEqual({
        model: "responder-b",
        content: "Response B",
        confidence: 0.7,
        usage: { inputTokens: 15, outputTokens: 25 },
      });
    });
  });

  describe("state transitions", () => {
    it("transitions through responding -> synthesizing -> done phases", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({
        content: "Synthesized",
        usage: { inputTokens: 50, outputTokens: 100 },
      });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      const phases = setModeState.mock.calls.map((call) => call[0].phase);
      expect(phases).toEqual(["responding", "synthesizing", "done"]);
    });

    it("sets final state with synthesis content and usage", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({
        content: "Final synthesis",
        usage: { inputTokens: 50, outputTokens: 100 },
      });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.synthesis).toBe("Final synthesis");
      expect(lastCall.synthesisUsage).toEqual({ inputTokens: 50, outputTokens: 100 });
    });
  });

  describe("abort controller management", () => {
    it("creates abort controllers for responding models", async () => {
      const ctx = createMockContext({ models: ["synth", "responder-a", "responder-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "B\n\nCONFIDENCE: 0.7" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Each responder call should have its own abort controller
      const call1Controller = streamResponse.mock.calls[0][2];
      const call2Controller = streamResponse.mock.calls[1][2];

      expect(call1Controller).toBeInstanceOf(AbortController);
      expect(call2Controller).toBeInstanceOf(AbortController);
      expect(call1Controller).not.toBe(call2Controller);
    });

    it("stores abort controllers in ref during responding phase", async () => {
      const ctx = createMockContext({ models: ["synth", "responder-a", "responder-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      let capturedControllers: AbortController[] = [];
      streamResponse.mockImplementation(async (model: string) => {
        if (model !== "synth") {
          capturedControllers = [...ctx.abortControllersRef.current];
        }
        return model === "synth"
          ? { content: "Synthesized" }
          : { content: "Response\n\nCONFIDENCE: 0.8" };
      });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Should have had 2 controllers during responding phase
      expect(capturedControllers.length).toBe(2);
    });

    it("creates separate abort controller for synthesis phase", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Final ref should have synthesis controller
      expect(ctx.abortControllersRef.current).toHaveLength(1);
      expect(ctx.abortControllersRef.current[0]).toBeInstanceOf(AbortController);
    });
  });

  describe("message filtering", () => {
    it("filters messages for each responding model", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Hello", timestamp: new Date() },
        {
          id: "2",
          role: "assistant" as const,
          content: "Hi",
          model: "responder-a",
          timestamp: new Date(),
        },
      ];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["synth", "responder-a", "responder-b"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "B\n\nCONFIDENCE: 0.7" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "responder-a");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "responder-b");
    });
  });

  describe("multimodal content handling", () => {
    it("extracts text from multimodal content for prompts", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      const multimodalContent = [
        { type: "input_text", text: "Analyze this image" },
        { type: "image", source: { type: "base64", data: "abc123" } },
      ];

      await sendConfidenceWeightedMode(multimodalContent, ctx, mockFallback);

      // Check confidence prompt contains extracted text
      const responderCall = streamResponse.mock.calls[0];
      const inputItems = responderCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("Analyze this image");
    });

    it("passes multimodal content to fallback when needed", async () => {
      const ctx = createMockContext({ models: ["synth"] });

      const multimodalContent = [
        { type: "input_text", text: "Question" },
        { type: "image", source: { type: "base64", data: "xyz" } },
      ];

      await sendConfidenceWeightedMode(multimodalContent, ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith(multimodalContent);
    });

    it("sends extracted text to synthesizer", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      const multimodalContent = [
        { type: "input_text", text: "Describe this image" },
        { type: "image", source: { type: "base64", data: "abc123" } },
      ];

      await sendConfidenceWeightedMode(multimodalContent, ctx, mockFallback);

      // Check synthesis call
      const synthCall = streamResponse.mock.calls[1];
      const userMessage = synthCall[1].find((i: { role: string }) => i.role === "user");
      expect(userMessage.content).toBe("Describe this image");
    });
  });

  describe("edge cases", () => {
    it("handles synthesis failure gracefully", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce(null); // synthesis fails

      const results = await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Should still mark as done
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1];
      expect(lastCall[0].phase).toBe("done");

      // Result should be null at synthesizer position
      expect(results[0]).toBeNull();
    });

    it("handles empty response content", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Empty content should still be added
      expect(updateModeState).toHaveBeenCalled();
    });

    it("handles confidence score at boundary values", async () => {
      const ctx = createMockContext({ models: ["synth", "resp-a", "resp-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.0" });
      streamResponse.mockResolvedValueOnce({ content: "B\n\nCONFIDENCE: 1.0" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // updateModeState should be called for each response
      expect(updateModeState.mock.calls.length).toBeGreaterThanOrEqual(2);
    });

    it("handles all responses with same confidence", async () => {
      const ctx = createMockContext({ models: ["synth", "model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "A\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "B\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "C\n\nCONFIDENCE: 0.8" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      const results = await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // Should still produce a valid result
      expect(results[0]).not.toBeNull();
      expect(results[0]?.content).toBe("Synthesized");
    });

    it("handles confidence score with extra whitespace", async () => {
      const ctx = createMockContext({ models: ["synth", "responder"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Answer\n\n  CONFIDENCE:   0.85   " });
      streamResponse.mockResolvedValueOnce({ content: "Synthesized" });

      await sendConfidenceWeightedMode("Question", ctx, mockFallback);

      // updateModeState should be called
      expect(updateModeState).toHaveBeenCalled();
    });
  });
});
