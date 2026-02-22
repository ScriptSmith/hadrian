import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendRoutedMode } from "../routed";
import { createMockContext as createBaseContext } from "./test-utils";

// Routed mode uses different default models
function createMockContext(overrides: Parameters<typeof createBaseContext>[0] = {}) {
  return createBaseContext({
    models: ["router-model", "target-a", "target-b"],
    ...overrides,
  });
}

// Helper to create mock fetch responses
function createMockRouterResponse(modelName: string, usage?: object, reasoning?: string) {
  const response: {
    output_text: string;
    output?: Array<{
      type: string;
      content?: Array<{ type: string; text?: string; reasoning_text?: string }>;
    }>;
    usage?: object;
  } = {
    output_text: modelName,
  };

  if (reasoning) {
    response.output = [
      {
        type: "reasoning",
        content: [{ type: "reasoning_text", text: reasoning }],
      },
    ];
  }

  if (usage) {
    response.usage = usage;
  }

  return response;
}

describe("sendRoutedMode", () => {
  let originalFetch: typeof globalThis.fetch;

  beforeEach(() => {
    originalFetch = globalThis.fetch;
    vi.resetAllMocks();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  describe("single model fallback", () => {
    it("directly uses the router model when no other models available", async () => {
      const ctx = createMockContext({ models: ["only-model"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "Direct response",
        usage: { inputTokens: 10, outputTokens: 20 },
      });

      const results = await sendRoutedMode("Hello", ctx);

      // Should initialize streaming with the single model (instance-aware with model map)
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(
        ["only-model"],
        new Map([["only-model", "only-model"]])
      );

      // Runner always calls setModeState for initialization, but should skip routing phase
      // The final state should show selectedModel as the router model
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      const finalCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(finalCall.selectedModel).toBe("only-model");
      expect(finalCall.phase).toBe("selected");

      // Should call streamResponse with instance-aware parameters
      expect(streamResponse).toHaveBeenCalledWith(
        "only-model", // model ID
        expect.any(Array),
        expect.any(AbortController),
        undefined, // settings
        "only-model", // instance ID as stream ID
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        undefined, // instance parameters
        undefined // instance label
      );

      // Should return the result
      expect(results).toHaveLength(1);
      expect(results[0]?.content).toBe("Direct response");
    });

    it("creates abort controller for direct mode", async () => {
      const ctx = createMockContext({ models: ["only-model"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      // Abort controller should be set
      expect(ctx.abortControllersRef.current).toHaveLength(1);
      expect(ctx.abortControllersRef.current[0]).toBeInstanceOf(AbortController);
    });
  });

  describe("router model selection", () => {
    it("uses first model as router by default", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-a")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response from A" });

      await sendRoutedMode("Hello", ctx);

      // Router request should be sent to first model
      const fetchCall = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
      const requestBody = JSON.parse(fetchCall[1].body);
      expect(requestBody.model).toBe("router");
    });

    it("uses configured router model", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { routerModel: "model-b" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("model-a")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response from A" });

      await sendRoutedMode("Hello", ctx);

      // Router request should be sent to configured model
      const fetchCall = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
      const requestBody = JSON.parse(fetchCall[1].body);
      expect(requestBody.model).toBe("model-b");
    });

    it("excludes router model from available targets", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-a")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      // Check that routing prompt contains only non-router models
      const fetchCall = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
      const requestBody = JSON.parse(fetchCall[1].body);
      const systemPrompt = requestBody.input[0].content;

      expect(systemPrompt).toContain("target-a");
      expect(systemPrompt).toContain("target-b");
      expect(systemPrompt).not.toContain("router\n"); // router should not be in the list
    });
  });

  describe("routing request", () => {
    it("sends routing request with correct parameters", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-a")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("What is the answer?", ctx);

      const fetchCall = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
      expect(fetchCall[0]).toBe("/api/v1/responses");

      const requestBody = JSON.parse(fetchCall[1].body);
      expect(requestBody.stream).toBe(false);
      expect(requestBody.max_output_tokens).toBe(100);
      expect(requestBody.temperature).toBe(0);
      expect(requestBody.input[1].content).toBe("What is the answer?");
    });

    it("uses custom routing prompt from config", async () => {
      const customPrompt = "Custom routing: select from these models";
      const ctx = createMockContext({
        models: ["router", "target"],
        modeConfig: { routingPrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      const fetchCall = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
      const requestBody = JSON.parse(fetchCall[1].body);
      expect(requestBody.input[0].content).toBe(customPrompt);
    });

    it("extracts text from multimodal content for routing", async () => {
      const ctx = createMockContext({ models: ["router", "target"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      const multimodalContent = [
        { type: "input_text", text: "Describe this image" },
        { type: "image", source: { type: "base64", data: "abc123" } },
      ];

      await sendRoutedMode(multimodalContent, ctx);

      const fetchCall = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
      const requestBody = JSON.parse(fetchCall[1].body);
      // User message should contain extracted text
      expect(requestBody.input[1].content).toBe("Describe this image");
    });
  });

  describe("model selection", () => {
    it("selects the model that matches the router output", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-b")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response from B" });

      const results = await sendRoutedMode("Hello", ctx);

      // Should stream from the selected instance
      expect(streamResponse).toHaveBeenCalledWith(
        "target-b", // model ID
        expect.any(Array),
        expect.any(AbortController),
        undefined, // settings
        "target-b", // instance ID as stream ID
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        undefined, // instance parameters
        undefined // instance label
      );

      // Result should be at target-b's position (index 2)
      expect(results[0]).toBeNull(); // router position
      expect(results[1]).toBeNull(); // target-a position
      expect(results[2]).not.toBeNull();
      expect(results[2]?.content).toBe("Response from B");
    });

    it("matches model names case-insensitively", async () => {
      const ctx = createMockContext({ models: ["router", "Claude-3-Opus", "GPT-4"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("CLAUDE-3-OPUS")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      expect(streamResponse).toHaveBeenCalledWith(
        "Claude-3-Opus", // model ID
        expect.any(Array),
        expect.any(AbortController),
        undefined, // settings
        "Claude-3-Opus", // instance ID as stream ID
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        undefined, // instance parameters
        undefined // instance label
      );
    });

    it("matches model name contained in response", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve(createMockRouterResponse("I think target-a would be best for this")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      expect(streamResponse).toHaveBeenCalledWith(
        "target-a", // model ID
        expect.any(Array),
        expect.any(AbortController),
        undefined, // settings
        "target-a", // instance ID as stream ID
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        undefined, // instance parameters
        undefined // instance label
      );
    });

    it("falls back to first target when router returns invalid model", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const consoleSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("unknown-model")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      // Should warn about unrecognized model
      expect(consoleSpy).toHaveBeenCalledWith(expect.stringContaining("unrecognized model"));

      // Should use first available target instance as fallback
      expect(streamResponse).toHaveBeenCalledWith(
        "target-a", // model ID
        expect.any(Array),
        expect.any(AbortController),
        undefined, // settings
        "target-a", // instance ID as stream ID
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        undefined, // instance parameters
        undefined // instance label
      );

      // Should mark as fallback in routing state
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      const finalCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(finalCall).toMatchObject({
        mode: "routed",
        phase: "selected",
        routerModel: "router",
        selectedModel: "target-a",
        reasoning: "unknown-model",
        isFallback: true,
      });

      consoleSpy.mockRestore();
    });
  });

  describe("routing errors", () => {
    it("falls back to first target when router request fails", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const consoleSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: false,
        statusText: "Internal Server Error",
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      expect(consoleSpy).toHaveBeenCalledWith("Routing failed, using fallback:", expect.any(Error));

      // Should use first target instance as fallback
      expect(streamResponse).toHaveBeenCalledWith(
        "target-a", // model ID
        expect.any(Array),
        expect.any(AbortController),
        undefined, // settings
        "target-a", // instance ID as stream ID
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        undefined, // instance parameters
        undefined // instance label
      );

      // Should set routing state with fallback info
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      const finalCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(finalCall).toMatchObject({
        mode: "routed",
        phase: "selected",
        routerModel: "router",
        selectedModel: "target-a",
        reasoning: "Routing failed, using default model",
        isFallback: true,
      });

      consoleSpy.mockRestore();
    });

    it("falls back when router request throws", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const consoleSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

      globalThis.fetch = vi.fn().mockRejectedValueOnce(new Error("Network error"));

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      expect(consoleSpy).toHaveBeenCalledWith("Routing failed, using fallback:", expect.any(Error));
      expect(streamResponse).toHaveBeenCalledWith(
        "target-a", // model ID
        expect.any(Array),
        expect.any(AbortController),
        undefined, // settings
        "target-a", // instance ID as stream ID
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        undefined, // instance parameters
        undefined // instance label
      );

      consoleSpy.mockRestore();
    });
  });

  describe("state transitions", () => {
    it("transitions through routing -> selected phases", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-b")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      // First call: routing phase
      expect(setModeState).toHaveBeenNthCalledWith(1, {
        mode: "routed",
        phase: "routing",
        routerModel: "router",
        routerInstanceId: "router",
        selectedModel: null,
        selectedInstanceId: null,
      });

      // Second call: selected phase
      expect(setModeState).toHaveBeenNthCalledWith(2, {
        mode: "routed",
        phase: "selected",
        routerModel: "router",
        routerInstanceId: "router",
        selectedModel: "target-b",
        selectedInstanceId: "target-b",
        reasoning: "target-b", // reasoning (the raw output)
        isFallback: false,
      });
    });

    it("initializes streaming for selected instance", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-b")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      // Instance-aware initialization with model map
      expect(ctx.streamingStore.initStreaming).toHaveBeenCalledWith(
        ["target-b"],
        new Map([["target-b", "target-b"]])
      );
    });
  });

  describe("reasoning extraction", () => {
    it("extracts reasoning from reasoning blocks", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            output_text: "target-a",
            output: [
              {
                type: "reasoning",
                content: [{ type: "reasoning_text", text: "I chose target-a because..." }],
              },
            ],
          }),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      // Reasoning should be extracted from the reasoning block
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      const finalCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(finalCall).toMatchObject({
        mode: "routed",
        phase: "selected",
        routerModel: "router",
        selectedModel: "target-a",
        reasoning: "I chose target-a because...",
        isFallback: false,
      });
    });

    it("uses raw output as reasoning when no reasoning block present", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            output_text: "target-a",
          }),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;
      const finalCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(finalCall).toMatchObject({
        mode: "routed",
        phase: "selected",
        routerModel: "router",
        selectedModel: "target-a",
        reasoning: "target-a",
        isFallback: false,
      });
    });
  });

  describe("usage tracking", () => {
    it("captures router usage in metadata", async () => {
      const ctx = createMockContext({ models: ["router", "target-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            output_text: "target-a",
            usage: {
              input_tokens: 50,
              output_tokens: 10,
              total_tokens: 60,
              cost: 0.001,
              input_tokens_details: { cached_tokens: 5 },
              output_tokens_details: { reasoning_tokens: 2 },
            },
          }),
      });

      streamResponse.mockResolvedValueOnce({
        content: "Response",
        usage: { inputTokens: 100, outputTokens: 200 },
      });

      const results = await sendRoutedMode("Hello", ctx);

      const metadata = results[1]?.modeMetadata;
      expect(metadata?.routerUsage).toEqual({
        inputTokens: 50,
        outputTokens: 10,
        totalTokens: 60,
        cost: 0.001,
        cachedTokens: 5,
        reasoningTokens: 2,
      });
    });
  });

  describe("metadata", () => {
    it("includes complete mode metadata in result", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            output_text: "target-b",
            usage: { input_tokens: 25, output_tokens: 5, total_tokens: 30 },
          }),
      });

      streamResponse.mockResolvedValueOnce({
        content: "Response from B",
        usage: { inputTokens: 100, outputTokens: 200 },
      });

      const results = await sendRoutedMode("Test question", ctx);

      const metadata = results[2]?.modeMetadata;
      expect(metadata).toEqual({
        mode: "routed",
        routerModel: "router",
        routingReasoning: "target-b",
        routerUsage: {
          inputTokens: 25,
          outputTokens: 5,
          totalTokens: 30,
          cost: undefined,
          cachedTokens: undefined,
          reasoningTokens: undefined,
        },
      });
    });
  });

  describe("message filtering", () => {
    it("filters messages for the selected target model", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Hello", timestamp: new Date() },
        {
          id: "2",
          role: "assistant" as const,
          content: "Hi",
          model: "target-a",
          timestamp: new Date(),
        },
      ];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["router", "target-a", "target-b"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-a")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Test", ctx);

      // Should filter messages for the selected model
      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "target-a");
    });

    it("filters messages for direct mode (single model)", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Hello", timestamp: new Date() },
      ];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["only-model"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Test", ctx);

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "only-model");
    });
  });

  describe("abort controller management", () => {
    it("sets abort controller for router request", async () => {
      const ctx = createMockContext({ models: ["router", "target-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-a")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      // Abort controllers should be managed throughout
      const fetchCall = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0];
      expect(fetchCall[1].signal).toBeInstanceOf(AbortSignal);
    });

    it("sets new abort controller for selected model streaming", async () => {
      const ctx = createMockContext({ models: ["router", "target-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-a")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response" });

      await sendRoutedMode("Hello", ctx);

      // Stream call should have its own abort controller
      const streamCall = streamResponse.mock.calls[0];
      expect(streamCall[2]).toBeInstanceOf(AbortController);
    });
  });

  describe("result mapping", () => {
    it("returns results in correct model positions", async () => {
      const ctx = createMockContext({ models: ["router", "target-a", "target-b", "target-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-c")),
      });

      streamResponse.mockResolvedValueOnce({ content: "Response from C" });

      const results = await sendRoutedMode("Hello", ctx);

      // Results array should match models array length
      expect(results).toHaveLength(4);

      // Only selected model position should have result
      expect(results[0]).toBeNull(); // router
      expect(results[1]).toBeNull(); // target-a
      expect(results[2]).toBeNull(); // target-b
      expect(results[3]).not.toBeNull(); // target-c
      expect(results[3]?.content).toBe("Response from C");
    });

    it("returns null when selected model fails", async () => {
      const ctx = createMockContext({ models: ["router", "target-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      globalThis.fetch = vi.fn().mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(createMockRouterResponse("target-a")),
      });

      streamResponse.mockResolvedValueOnce(null); // Selected model fails

      const results = await sendRoutedMode("Hello", ctx);

      expect(results).toEqual([null, null]);
    });
  });
});

// Import afterEach for cleanup
import { afterEach } from "vitest";
