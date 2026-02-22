import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendSynthesizedMode } from "../synthesized";
import {
  createMockContext,
  createMockFallback,
  createTestMessages,
  testMultimodalContent,
} from "./test-utils";

describe("sendSynthesizedMode", () => {
  let mockFallback: ReturnType<typeof createMockFallback>;

  beforeEach(() => {
    mockFallback = createMockFallback();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode when only one model (the synthesizer)", async () => {
      const ctx = createMockContext({ models: ["model-a"] });

      await sendSynthesizedMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
      expect(ctx.streamResponse).not.toHaveBeenCalled();
    });

    it("falls back when synthesizer is the only model via config", async () => {
      const ctx = createMockContext({
        models: ["synthesizer-model"],
        modeConfig: { synthesizerModel: "synthesizer-model" },
      });

      await sendSynthesizedMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
    });
  });

  describe("synthesizer model selection", () => {
    it("uses first model as synthesizer by default", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({
          content: "Response B",
          usage: { inputTokens: 10, outputTokens: 20 },
        })
        .mockResolvedValueOnce({
          content: "Response C",
          usage: { inputTokens: 15, outputTokens: 25 },
        })
        .mockResolvedValueOnce({
          content: "Synthesized response",
          usage: { inputTokens: 50, outputTokens: 100 },
        });

      const results = await sendSynthesizedMode("Hello", ctx, mockFallback);

      // First two calls are for model-b and model-c (responding instances)
      // The runner's gatherInstances passes: model, inputItems, controller, settings, instanceId, trackToolCalls, onSSEEvent, instanceParams, instanceLabel
      expect(streamResponse).toHaveBeenNthCalledWith(
        1,
        "model-b",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-b", // instanceId (same as model when derived from models)
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        undefined, // instanceParams (no params when derived from models)
        undefined // instanceLabel (no label when derived from models)
      );
      expect(streamResponse).toHaveBeenNthCalledWith(
        2,
        "model-c",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-c", // instanceId
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );

      // Third call is for model-a (synthesizer) via streamInstance
      expect(streamResponse).toHaveBeenNthCalledWith(
        3,
        "model-a",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-a", // instanceId
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );

      // Result should be at position 0 (model-a's position)
      expect(results[0]).not.toBeNull();
      expect(results[0]?.content).toBe("Synthesized response");
      expect(results[1]).toBeNull();
      expect(results[2]).toBeNull();
    });

    it("uses configured synthesizer model", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { synthesizerModel: "model-c" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response A" })
        .mockResolvedValueOnce({ content: "Response B" })
        .mockResolvedValueOnce({ content: "Synthesized" });

      const results = await sendSynthesizedMode("Hello", ctx, mockFallback);

      // First two calls are for model-a and model-b (responding instances)
      // The runner's gatherInstances passes full instance-aware args
      expect(streamResponse).toHaveBeenNthCalledWith(
        1,
        "model-a",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-a", // instanceId
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
      expect(streamResponse).toHaveBeenNthCalledWith(
        2,
        "model-b",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-b", // instanceId
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );

      // Third call is for model-c (configured synthesizer) via streamInstance
      expect(streamResponse).toHaveBeenNthCalledWith(
        3,
        "model-c",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "model-c", // instanceId
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );

      // Result at position 2 (model-c's position)
      expect(results[2]).not.toBeNull();
      expect(results[0]).toBeNull();
      expect(results[1]).toBeNull();
    });
  });

  describe("response gathering", () => {
    it("gathers responses from all non-synthesizer models", async () => {
      const ctx = createMockContext({ models: ["synth", "worker-1", "worker-2"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({
          content: "Worker 1 response",
          usage: { inputTokens: 10, outputTokens: 20 },
        })
        .mockResolvedValueOnce({
          content: "Worker 2 response",
          usage: { inputTokens: 15, outputTokens: 25 },
        })
        .mockResolvedValueOnce({ content: "Final synthesis" });

      await sendSynthesizedMode("Test question", ctx, mockFallback);

      // Verify streaming was initialized for worker instances (with model map)
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(
        ["worker-1", "worker-2"],
        new Map([
          ["worker-1", "worker-1"],
          ["worker-2", "worker-2"],
        ])
      );

      // Verify state updates via setModeState (runner uses setModeState for all updates)
      // Initial state + 2 worker completions + synthesizing state + done state
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      expect(setModeState.mock.calls.length).toBeGreaterThanOrEqual(4);
    });

    it("handles partial failures gracefully", async () => {
      const ctx = createMockContext({ models: ["synth", "worker-1", "worker-2", "worker-3"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Worker 1 response" })
        .mockResolvedValueOnce(null) // worker-2 fails
        .mockResolvedValueOnce({ content: "Worker 3 response" })
        .mockResolvedValueOnce({ content: "Synthesized" });

      const results = await sendSynthesizedMode("Test", ctx, mockFallback);

      // Should still synthesize with successful responses
      expect(results[0]).not.toBeNull();
      expect(results[0]?.content).toBe("Synthesized");

      // Verify state updates via setModeState (runner uses setModeState for all updates)
      // Initial state + 2 successful worker completions + synthesizing state + done state
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      expect(setModeState.mock.calls.length).toBeGreaterThanOrEqual(4);
    });

    it("returns empty results when all workers fail", async () => {
      const ctx = createMockContext({ models: ["synth", "worker-1", "worker-2"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue(null); // All fail

      const results = await sendSynthesizedMode("Test", ctx, mockFallback);

      // Should return array of nulls
      expect(results).toEqual([null, null, null]);
      // Synthesis should not be called
      expect(streamResponse).toHaveBeenCalledTimes(2); // Only worker calls, no synthesis
    });
  });

  describe("synthesis phase", () => {
    it("uses default synthesis prompt with responses formatted", async () => {
      const ctx = createMockContext({ models: ["synth", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response from A" })
        .mockResolvedValueOnce({ content: "Response from B" })
        .mockResolvedValueOnce({ content: "Synthesized" });

      await sendSynthesizedMode("What is the answer?", ctx, mockFallback);

      // Check the synthesis call
      const synthCall = streamResponse.mock.calls[2];
      const inputItems = synthCall[1];

      // First item should be system prompt with responses
      expect(inputItems[0].role).toBe("system");
      expect(inputItems[0].content).toContain("[worker-a]:");
      expect(inputItems[0].content).toContain("Response from A");
      expect(inputItems[0].content).toContain("[worker-b]:");
      expect(inputItems[0].content).toContain("Response from B");

      // Second item should be user message
      expect(inputItems[1].role).toBe("user");
      expect(inputItems[1].content).toBe("What is the answer?");
    });

    it("uses custom synthesis prompt from config", async () => {
      const customPrompt = "Custom prompt: synthesize these: {responses}";
      const ctx = createMockContext({
        models: ["synth", "worker"],
        modeConfig: { synthesisPrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Worker response" })
        .mockResolvedValueOnce({ content: "Synthesized" });

      await sendSynthesizedMode("Test", ctx, mockFallback);

      const synthCall = streamResponse.mock.calls[1];
      const systemPrompt = synthCall[1][0].content;

      // Should use custom prompt (note: {responses} isn't replaced when custom prompt is used)
      expect(systemPrompt).toBe(customPrompt);
    });

    it("extracts text from multimodal content for synthesis", async () => {
      const ctx = createMockContext({ models: ["synth", "worker"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Worker response" })
        .mockResolvedValueOnce({ content: "Synthesized" });

      await sendSynthesizedMode(testMultimodalContent, ctx, mockFallback);

      const synthCall = streamResponse.mock.calls[1];
      const userMessage = synthCall[1][1];

      // User message should be extracted text
      expect(userMessage.content).toBe("Describe this image");
    });
  });

  describe("state transitions", () => {
    it("transitions through gathering -> synthesizing -> done phases", async () => {
      const ctx = createMockContext({ models: ["synth", "worker"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Worker response" })
        .mockResolvedValueOnce({ content: "Synthesized" });

      await sendSynthesizedMode("Test", ctx, mockFallback);

      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      const calls = setModeState.mock.calls;

      // Verify initial gathering state is set first
      expect(calls[0][0]).toEqual({
        mode: "synthesized",
        phase: "gathering",
        synthesizerModel: "synth",
        synthesizerInstanceId: "synth",
        completedModels: [],
        totalModels: 1,
        sourceResponses: [],
      });

      // Find the synthesizing state call
      const synthesizingCall = calls.find(
        (call) => call[0].mode === "synthesized" && call[0].phase === "synthesizing"
      )!;
      expect(synthesizingCall).toBeDefined();
      expect(synthesizingCall[0]).toMatchObject({
        mode: "synthesized",
        phase: "synthesizing",
        synthesizerModel: "synth",
        completedModels: ["worker"],
        totalModels: 1,
      });
      expect(synthesizingCall[0].sourceResponses).toHaveLength(1);
      expect(synthesizingCall[0].sourceResponses[0].model).toBe("worker");
      expect(synthesizingCall[0].sourceResponses[0].content).toBe("Worker response");

      // Find the final done state call
      const doneCall = calls.find(
        (call) => call[0].mode === "synthesized" && call[0].phase === "done"
      )!;
      expect(doneCall).toBeDefined();
      expect(doneCall[0]).toMatchObject({
        mode: "synthesized",
        phase: "done",
        synthesizerModel: "synth",
        completedModels: ["worker"],
        totalModels: 1,
      });
      expect(doneCall[0].sourceResponses).toHaveLength(1);
    });
  });

  describe("metadata", () => {
    it("includes source responses in metadata", async () => {
      const ctx = createMockContext({ models: ["synth", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({
          content: "Response A",
          usage: { inputTokens: 10, outputTokens: 20 },
        })
        .mockResolvedValueOnce({
          content: "Response B",
          usage: { inputTokens: 15, outputTokens: 25 },
        })
        .mockResolvedValueOnce({
          content: "Synthesized",
          usage: { inputTokens: 50, outputTokens: 100 },
        });

      const results = await sendSynthesizedMode("Test", ctx, mockFallback);

      const synthResult = results[0];
      expect(synthResult?.modeMetadata).toBeDefined();
      expect(synthResult?.modeMetadata?.mode).toBe("synthesized");
      expect(synthResult?.modeMetadata?.isSynthesized).toBe(true);
      expect(synthResult?.modeMetadata?.synthesizerModel).toBe("synth");
      expect(synthResult?.modeMetadata?.synthesizerUsage).toEqual({
        inputTokens: 50,
        outputTokens: 100,
      });

      const sourceResponses = synthResult?.modeMetadata?.sourceResponses;
      expect(sourceResponses).toHaveLength(2);
      expect(sourceResponses?.[0]).toEqual({
        model: "worker-a",
        content: "Response A",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      expect(sourceResponses?.[1]).toEqual({
        model: "worker-b",
        content: "Response B",
        usage: { inputTokens: 15, outputTokens: 25 },
      });
    });
  });

  describe("abort controller management", () => {
    it("creates abort controllers for worker models", async () => {
      const ctx = createMockContext({ models: ["synth", "worker-1", "worker-2"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Worker 1" })
        .mockResolvedValueOnce({ content: "Worker 2" })
        .mockResolvedValueOnce({ content: "Synthesized" });

      await sendSynthesizedMode("Test", ctx, mockFallback);

      // Each stream call should receive a unique AbortController
      const call1Controller = streamResponse.mock.calls[0][2];
      const call2Controller = streamResponse.mock.calls[1][2];
      const call3Controller = streamResponse.mock.calls[2][2];

      expect(call1Controller).toBeInstanceOf(AbortController);
      expect(call2Controller).toBeInstanceOf(AbortController);
      expect(call3Controller).toBeInstanceOf(AbortController);
      expect(call1Controller).not.toBe(call2Controller);
    });
  });

  describe("message filtering", () => {
    it("filters messages for each worker model", async () => {
      const messages = createTestMessages();
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["synth", "worker-1", "worker-2"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Worker 1" })
        .mockResolvedValueOnce({ content: "Worker 2" })
        .mockResolvedValueOnce({ content: "Synthesized" });

      await sendSynthesizedMode("Test", ctx, mockFallback);

      // Should filter messages for each worker model
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "worker-1");
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "worker-2");
    });
  });
});
