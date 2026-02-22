import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendScattershotMode, DEFAULT_SCATTERSHOT_VARIATIONS } from "../scattershot";
import { createMockContext, createMockFallback } from "./test-utils";

describe("sendScattershotMode", () => {
  let mockFallback: ReturnType<typeof createMockFallback>;

  beforeEach(() => {
    mockFallback = createMockFallback();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode when no models provided", async () => {
      const ctx = createMockContext({ models: [] });

      await sendScattershotMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
      expect(ctx.streamResponse).not.toHaveBeenCalled();
    });
  });

  describe("target model selection", () => {
    it("uses first model as target", async () => {
      const ctx = createMockContext({ models: ["target-model", "other-model"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      await sendScattershotMode("Hello", ctx, mockFallback);

      // All calls should be to the target model
      expect(streamResponse).toHaveBeenCalledTimes(DEFAULT_SCATTERSHOT_VARIATIONS.length);
      for (const call of streamResponse.mock.calls) {
        expect(call[0]).toBe("target-model");
      }
    });
  });

  describe("default parameter variations", () => {
    it("uses default variations when none configured", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      expect(streamResponse).toHaveBeenCalledTimes(4); // DEFAULT_SCATTERSHOT_VARIATIONS has 4

      // Check that each variation uses the correct params
      const settingsUsed = streamResponse.mock.calls.map((call) => call[3]);
      expect(settingsUsed[0]).toMatchObject({ temperature: 0.0 });
      expect(settingsUsed[1]).toMatchObject({ temperature: 0.5 });
      expect(settingsUsed[2]).toMatchObject({ temperature: 1.0 });
      expect(settingsUsed[3]).toMatchObject({ temperature: 1.5, topP: 0.9 });
    });

    it("uses custom variations from config", async () => {
      const customVariations = [
        { temperature: 0.2, topK: 40 },
        { temperature: 0.8, topK: 80 },
      ];
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { parameterVariations: customVariations },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      expect(streamResponse).toHaveBeenCalledTimes(2);

      const settingsUsed = streamResponse.mock.calls.map((call) => call[3]);
      expect(settingsUsed[0]).toMatchObject({ temperature: 0.2, topK: 40 });
      expect(settingsUsed[1]).toMatchObject({ temperature: 0.8, topK: 80 });
    });

    it("ignores empty custom variations and uses defaults", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { parameterVariations: [] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      expect(streamResponse).toHaveBeenCalledTimes(DEFAULT_SCATTERSHOT_VARIATIONS.length);
    });
  });

  describe("variation labels", () => {
    it("generates labels from temperature parameter", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      // Check labels are generated correctly
      expect(results[0]?.variationLabel).toBe("temp=0");
      expect(results[1]?.variationLabel).toBe("temp=0.5");
      expect(results[2]?.variationLabel).toBe("temp=1");
      expect(results[3]?.variationLabel).toBe("temp=1.5, top_p=0.9");
    });

    it("generates labels for all parameter types", async () => {
      const customVariations = [
        {
          temperature: 0.5,
          topP: 0.9,
          topK: 40,
          frequencyPenalty: 0.5,
          presencePenalty: 0.3,
          maxTokens: 1000,
        },
      ];
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { parameterVariations: customVariations },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.variationLabel).toBe(
        "temp=0.5, top_p=0.9, top_k=40, freq=0.5, pres=0.3, max=1000"
      );
    });

    it("uses fallback label for empty parameters", async () => {
      const customVariations = [{}];
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { parameterVariations: customVariations },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.variationLabel).toBe("Variation 1");
    });
  });

  describe("variation IDs", () => {
    it("generates unique variation IDs", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      const ids = results.map((r) => r?.variationId);
      expect(ids[0]).toBe("model-a__variation_0");
      expect(ids[1]).toBe("model-a__variation_1");
      expect(ids[2]).toBe("model-a__variation_2");
      expect(ids[3]).toBe("model-a__variation_3");
    });
  });

  describe("streaming initialization", () => {
    it("initializes streaming with variation IDs and model map", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      // Should pass both variation IDs and a model map
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;
      expect(initStreaming).toHaveBeenCalledTimes(1);

      const [ids, modelMap] = initStreaming.mock.calls[0];
      expect(ids).toEqual([
        "model-a__variation_0",
        "model-a__variation_1",
        "model-a__variation_2",
        "model-a__variation_3",
      ]);
      expect(modelMap).toBeInstanceOf(Map);
      expect(modelMap.get("model-a__variation_0")).toBe("model-a");
      expect(modelMap.get("model-a__variation_3")).toBe("model-a");
    });
  });

  describe("parallel execution", () => {
    it("runs all variations in parallel", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      let concurrentCalls = 0;
      let maxConcurrent = 0;

      streamResponse.mockImplementation(async () => {
        concurrentCalls++;
        maxConcurrent = Math.max(maxConcurrent, concurrentCalls);
        await new Promise((resolve) => setTimeout(resolve, 10));
        concurrentCalls--;
        return { content: "Response" };
      });

      await sendScattershotMode("Hello", ctx, mockFallback);

      expect(maxConcurrent).toBe(DEFAULT_SCATTERSHOT_VARIATIONS.length);
    });
  });

  describe("response handling", () => {
    it("captures successful responses", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({
          content: "Response 0",
          usage: { inputTokens: 10, outputTokens: 20 },
        })
        .mockResolvedValueOnce({
          content: "Response 1",
          usage: { inputTokens: 15, outputTokens: 25 },
        })
        .mockResolvedValueOnce({
          content: "Response 2",
          usage: { inputTokens: 20, outputTokens: 30 },
        })
        .mockResolvedValueOnce({
          content: "Response 3",
          usage: { inputTokens: 25, outputTokens: 35 },
        });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results).toHaveLength(4);
      expect(results[0]?.content).toBe("Response 0");
      expect(results[1]?.content).toBe("Response 1");
      expect(results[2]?.content).toBe("Response 2");
      expect(results[3]?.content).toBe("Response 3");
    });

    it("handles failed variations (returns null)", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response 0" })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce({ content: "Response 2" })
        .mockResolvedValueOnce(null);

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results).toHaveLength(4);
      expect(results[0]?.content).toBe("Response 0");
      expect(results[1]).toBeNull();
      expect(results[2]?.content).toBe("Response 2");
      expect(results[3]).toBeNull();
    });

    it("handles thrown errors gracefully", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response 0" })
        .mockRejectedValueOnce(new Error("Network error"))
        .mockResolvedValueOnce({ content: "Response 2" })
        .mockResolvedValueOnce({ content: "Response 3" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results).toHaveLength(4);
      expect(results[0]?.content).toBe("Response 0");
      expect(results[1]).toBeNull();
      expect(results[2]?.content).toBe("Response 2");
      expect(results[3]?.content).toBe("Response 3");
    });

    it("handles all variations failing", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue(null);

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results).toHaveLength(4);
      expect(results.every((r) => r === null)).toBe(true);
    });
  });

  describe("state management", () => {
    it("sets initial scattershot state", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      // First call is initialization
      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.mode).toBe("scattershot");
      expect(firstCall.phase).toBe("generating");
      expect(firstCall.targetModel).toBe("model-a");
      expect(firstCall.variations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            id: "model-a__variation_0",
            index: 0,
            status: "pending",
          }),
        ])
      );
    });

    it("sets done state with completed variations", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      await sendScattershotMode("Hello", ctx, mockFallback);

      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      // Last call is done state
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.targetModel).toBe("model-a");
      expect(lastCall.variations).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            status: "complete",
            content: "Response",
            usage: { inputTokens: 10, outputTokens: 20 },
          }),
        ])
      );
    });

    it("marks failed variations in done state", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "Response" })
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce({ content: "Response" })
        .mockResolvedValueOnce(null);

      await sendScattershotMode("Hello", ctx, mockFallback);

      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.variations[0].status).toBe("complete");
      expect(lastCall.variations[1].status).toBe("failed");
      expect(lastCall.variations[2].status).toBe("complete");
      expect(lastCall.variations[3].status).toBe("failed");
    });
  });

  describe("variation status updates", () => {
    it("updates variation to generating status when starting", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      // updateModeState is called to mark variations as generating
      expect(updateModeState).toHaveBeenCalled();

      // Verify updater function sets status to generating
      const mockState = {
        mode: "scattershot" as const,
        phase: "generating" as const,
        targetModel: "model-a",
        variations: [{ id: "model-a__variation_0", status: "pending", index: 0, label: "temp=0" }],
      };
      const firstUpdater = updateModeState.mock.calls[0][0];
      const afterUpdate = firstUpdater(mockState);
      expect(afterUpdate.variations[0].status).toBe("generating");
    });

    it("updates variation to complete status with content and usage", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      await sendScattershotMode("Hello", ctx, mockFallback);

      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      // updateModeState is called multiple times
      expect(updateModeState.mock.calls.length).toBeGreaterThanOrEqual(4); // generating + complete per variation
    });

    it("updates variation to failed status on error", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockRejectedValue(new Error("Failed"));

      await sendScattershotMode("Hello", ctx, mockFallback);

      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      // Check the final done state has failed variations
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.variations[0].status).toBe("failed");
    });

    it("updates variation to failed status when result is null", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue(null);

      await sendScattershotMode("Hello", ctx, mockFallback);

      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      // Check the final done state has failed variations
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.variations[0].status).toBe("failed");
    });
  });

  describe("settings merging", () => {
    it("merges base settings with variation params", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        settings: { systemPrompt: "You are helpful", maxTokens: 2000 },
        modeConfig: { parameterVariations: [{ temperature: 0.7 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      // Settings should be merged with variation params
      expect(streamResponse).toHaveBeenCalledWith(
        "model-a",
        expect.any(Array),
        expect.any(AbortController),
        expect.objectContaining({
          systemPrompt: "You are helpful",
          maxTokens: 2000,
          temperature: 0.7,
        }),
        expect.any(String)
      );
    });

    it("variation params override base settings", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        settings: { temperature: 0.5, maxTokens: 2000 },
        modeConfig: { parameterVariations: [{ temperature: 0.9, topP: 0.8 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      expect(streamResponse).toHaveBeenCalledWith(
        "model-a",
        expect.any(Array),
        expect.any(AbortController),
        expect.objectContaining({
          temperature: 0.9, // Overridden by variation
          maxTokens: 2000, // From base settings
          topP: 0.8, // From variation
        }),
        expect.any(String)
      );
    });
  });

  describe("abort controller management", () => {
    it("creates abort controllers for each variation", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      // Each call should get its own abort controller
      const controllers = streamResponse.mock.calls.map((call) => call[2]);
      expect(controllers).toHaveLength(4);
      expect(controllers[0]).toBeInstanceOf(AbortController);
      expect(controllers[1]).toBeInstanceOf(AbortController);
      expect(controllers[2]).toBeInstanceOf(AbortController);
      expect(controllers[3]).toBeInstanceOf(AbortController);

      // All controllers should be unique
      const uniqueControllers = new Set(controllers);
      expect(uniqueControllers.size).toBe(4);
    });

    it("stores controllers in abortControllersRef", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      expect(ctx.abortControllersRef.current).toHaveLength(4);
      expect(ctx.abortControllersRef.current[0]).toBeInstanceOf(AbortController);
    });
  });

  describe("message filtering", () => {
    it("filters messages for target model", async () => {
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
        models: ["model-a"],
        messages,
        filterMessagesForModel,
        modeConfig: { parameterVariations: [{ temperature: 0.5 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Test", ctx, mockFallback);

      // Should filter messages for the target model
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "model-a");
    });
  });

  describe("multimodal content handling", () => {
    it("extracts text from multimodal content", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        modeConfig: { parameterVariations: [{ temperature: 0.5 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const multimodalContent = [
        { type: "input_text", text: "Describe this image" },
        { type: "image", source: { type: "base64", data: "abc123" } },
      ];

      await sendScattershotMode(multimodalContent, ctx, mockFallback);

      const inputItems = streamResponse.mock.calls[0][1];
      const userMessage = inputItems[inputItems.length - 1];
      expect(userMessage.content).toBe("Describe this image");
    });
  });

  describe("result metadata", () => {
    it("includes mode metadata in results", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.mode).toBe("scattershot");
      expect(results[0]?.modeMetadata?.isScattershot).toBe(true);
      expect(results[0]?.modeMetadata?.scattershotModel).toBe("model-a");
    });

    it("includes variation label in result metadata", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.scattershotVariationLabel).toBe("temp=0");
      expect(results[1]?.modeMetadata?.scattershotVariationLabel).toBe("temp=0.5");
    });

    it("includes variation params in result metadata", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.scattershotVariationParams).toEqual({ temperature: 0.0 });
      expect(results[3]?.modeMetadata?.scattershotVariationParams).toEqual({
        temperature: 1.5,
        topP: 0.9,
      });
    });

    it("includes all variations only on first result", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: "Response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      // First result should have all variations
      expect(results[0]?.modeMetadata?.scattershotVariations).toHaveLength(4);
      expect(results[0]?.modeMetadata?.scattershotVariations?.[0]).toMatchObject({
        id: "model-a__variation_0",
        index: 0,
        label: "temp=0",
        content: "Response",
      });

      // Other results should not have all variations
      expect(results[1]?.modeMetadata?.scattershotVariations).toBeUndefined();
      expect(results[2]?.modeMetadata?.scattershotVariations).toBeUndefined();
      expect(results[3]?.modeMetadata?.scattershotVariations).toBeUndefined();
    });

    it("includes aggregate usage only on first result", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "R0", usage: { inputTokens: 10, outputTokens: 20 } })
        .mockResolvedValueOnce({ content: "R1", usage: { inputTokens: 15, outputTokens: 25 } })
        .mockResolvedValueOnce({ content: "R2", usage: { inputTokens: 20, outputTokens: 30 } })
        .mockResolvedValueOnce({ content: "R3", usage: { inputTokens: 25, outputTokens: 35 } });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      // First result should have aggregate usage
      expect(results[0]?.modeMetadata?.aggregateUsage).toMatchObject({
        inputTokens: 70, // 10 + 15 + 20 + 25
        outputTokens: 110, // 20 + 25 + 30 + 35
      });

      // Other results should not have aggregate usage
      expect(results[1]?.modeMetadata?.aggregateUsage).toBeUndefined();
    });

    it("includes usage on individual results", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse
        .mockResolvedValueOnce({ content: "R0", usage: { inputTokens: 10, outputTokens: 20 } })
        .mockResolvedValueOnce({ content: "R1", usage: { inputTokens: 15, outputTokens: 25 } })
        .mockResolvedValueOnce({ content: "R2", usage: { inputTokens: 20, outputTokens: 30 } })
        .mockResolvedValueOnce({ content: "R3", usage: { inputTokens: 25, outputTokens: 35 } });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.usage).toEqual({ inputTokens: 10, outputTokens: 20 });
      expect(results[1]?.usage).toEqual({ inputTokens: 15, outputTokens: 25 });
      expect(results[2]?.usage).toEqual({ inputTokens: 20, outputTokens: 30 });
      expect(results[3]?.usage).toEqual({ inputTokens: 25, outputTokens: 35 });
    });
  });

  describe("result structure", () => {
    it("returns one result per variation", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results).toHaveLength(DEFAULT_SCATTERSHOT_VARIATIONS.length);
    });

    it("results include variation ID", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.variationId).toBe("model-a__variation_0");
      expect(results[1]?.variationId).toBe("model-a__variation_1");
    });

    it("results include variation label", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.variationLabel).toBe("temp=0");
      expect(results[3]?.variationLabel).toBe("temp=1.5, top_p=0.9");
    });
  });

  describe("streaming with variation ID", () => {
    it("passes variation ID to streamResponse for streaming store", async () => {
      const ctx = createMockContext({ models: ["model-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      // The 5th argument to streamResponse should be the variation ID
      expect(streamResponse.mock.calls[0][4]).toBe("model-a__variation_0");
      expect(streamResponse.mock.calls[1][4]).toBe("model-a__variation_1");
      expect(streamResponse.mock.calls[2][4]).toBe("model-a__variation_2");
      expect(streamResponse.mock.calls[3][4]).toBe("model-a__variation_3");
    });
  });

  describe("instance support", () => {
    it("uses first instance for variations when instances provided", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b"],
        instances: [
          { id: "instance-1", modelId: "model-a", label: "Creative GPT" },
          { id: "instance-2", modelId: "model-b", label: "Analytical GPT" },
        ],
        modeConfig: { parameterVariations: [{ temperature: 0.5 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      // Should use the first instance's model
      expect(streamResponse.mock.calls[0][0]).toBe("model-a");
    });

    it("uses instance ID for variation IDs when instances provided", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        instances: [{ id: "my-instance", modelId: "model-a", label: "Test Instance" }],
        modeConfig: { parameterVariations: [{ temperature: 0.5 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      // Variation ID should use instance ID
      expect(results[0]?.variationId).toBe("my-instance__variation_0");
    });

    it("merges instance parameters with variation params", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        instances: [
          {
            id: "instance-1",
            modelId: "model-a",
            parameters: { maxTokens: 1000, topP: 0.8 },
          },
        ],
        settings: { systemPrompt: "Base prompt" },
        modeConfig: { parameterVariations: [{ temperature: 0.7 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      // Settings should merge: base -> instance -> variation
      expect(streamResponse).toHaveBeenCalledWith(
        "model-a",
        expect.any(Array),
        expect.any(AbortController),
        expect.objectContaining({
          systemPrompt: "Base prompt", // From base settings
          maxTokens: 1000, // From instance params
          topP: 0.8, // From instance params
          temperature: 0.7, // From variation (overrides)
        }),
        expect.any(String)
      );
    });

    it("variation params override instance params", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        instances: [
          {
            id: "instance-1",
            modelId: "model-a",
            parameters: { temperature: 0.3, maxTokens: 500 },
          },
        ],
        modeConfig: { parameterVariations: [{ temperature: 0.9 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      await sendScattershotMode("Hello", ctx, mockFallback);

      // Variation temperature should override instance temperature
      expect(streamResponse).toHaveBeenCalledWith(
        "model-a",
        expect.any(Array),
        expect.any(AbortController),
        expect.objectContaining({
          temperature: 0.9, // From variation (overrides instance)
          maxTokens: 500, // From instance params
        }),
        expect.any(String)
      );
    });

    it("includes instance label in result metadata", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        instances: [{ id: "instance-1", modelId: "model-a", label: "My Custom Instance" }],
        modeConfig: { parameterVariations: [{ temperature: 0.5 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.scattershotInstanceLabel).toBe("My Custom Instance");
    });

    it("falls back to model ID for instance label when not set", async () => {
      const ctx = createMockContext({
        models: ["model-a"],
        instances: [{ id: "instance-1", modelId: "model-a" }],
        modeConfig: { parameterVariations: [{ temperature: 0.5 }] },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: "Response" });

      const results = await sendScattershotMode("Hello", ctx, mockFallback);

      expect(results[0]?.modeMetadata?.scattershotInstanceLabel).toBe("model-a");
    });
  });
});

describe("DEFAULT_SCATTERSHOT_VARIATIONS", () => {
  it("contains expected default variations", () => {
    expect(DEFAULT_SCATTERSHOT_VARIATIONS).toHaveLength(4);
    expect(DEFAULT_SCATTERSHOT_VARIATIONS[0]).toEqual({ temperature: 0.0 });
    expect(DEFAULT_SCATTERSHOT_VARIATIONS[1]).toEqual({ temperature: 0.5 });
    expect(DEFAULT_SCATTERSHOT_VARIATIONS[2]).toEqual({ temperature: 1.0 });
    expect(DEFAULT_SCATTERSHOT_VARIATIONS[3]).toEqual({ temperature: 1.5, topP: 0.9 });
  });
});
