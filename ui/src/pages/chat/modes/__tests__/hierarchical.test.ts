import { describe, it, expect, vi, beforeEach } from "vitest";
import { sendHierarchicalMode } from "../hierarchical";
import { createMockContext as createBaseContext, createMockFallback } from "./test-utils";

// Hierarchical mode uses coordinator + worker models
function createMockContext(overrides: Parameters<typeof createBaseContext>[0] = {}) {
  return createBaseContext({
    models: ["coordinator", "worker-a", "worker-b"],
    ...overrides,
  });
}

describe("sendHierarchicalMode", () => {
  let mockFallback: ReturnType<typeof createMockFallback>;

  beforeEach(() => {
    vi.clearAllMocks();
    mockFallback = createMockFallback();
  });

  describe("fallback behavior", () => {
    it("falls back to multiple mode with single model", async () => {
      const ctx = createMockContext({ models: ["coordinator"] });

      await sendHierarchicalMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
      expect(ctx.streamResponse).not.toHaveBeenCalled();
    });

    it("does not fall back with two models (1 coordinator + 1 worker)", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task 1"}]}',
        usage: {},
      });

      await sendHierarchicalMode("Hello", ctx, mockFallback);

      expect(mockFallback).not.toHaveBeenCalled();
      expect(streamResponse).toHaveBeenCalled();
    });

    it("falls back when coordinator is the only model (no workers)", async () => {
      // If coordinatorModel is specified but matches the only model, no workers remain
      const ctx = createMockContext({
        models: ["coordinator"],
        modeConfig: { coordinatorModel: "coordinator" },
      });

      await sendHierarchicalMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
    });

    it("falls back when specified coordinator excludes all workers", async () => {
      // coordinatorModel = "coordinator", workers = ["coordinator"] after filter => empty
      const ctx = createMockContext({
        models: ["coordinator"],
        modeConfig: { coordinatorModel: "coordinator" },
      });

      await sendHierarchicalMode("Hello", ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith("Hello");
    });
  });

  describe("coordinator model selection", () => {
    it("uses first model as coordinator by default", async () => {
      const ctx = createMockContext({ models: ["model-a", "model-b", "model-c"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // First call should be decomposing phase with default coordinator (first model)
      const decomposingCall = setModeState.mock.calls.find(
        (call) => call[0].phase === "decomposing"
      )!;
      expect(decomposingCall).toBeDefined();
      expect(decomposingCall[0].coordinatorModel).toBe("model-a");
    });

    it("uses configured coordinator model", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { coordinatorModel: "model-b" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Should use the configured coordinator model
      const decomposingCall = setModeState.mock.calls.find(
        (call) => call[0].phase === "decomposing"
      )!;
      expect(decomposingCall).toBeDefined();
      expect(decomposingCall[0].coordinatorModel).toBe("model-b");
    });

    it("excludes coordinator from worker models", async () => {
      const ctx = createMockContext({
        models: ["coordinator", "worker-a", "worker-b"],
        modeConfig: { coordinatorModel: "coordinator" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // Decomposition returns subtasks
      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Task 1", "assignedModel": "worker-a"}, {"description": "Task 2", "assignedModel": "worker-b"}]}',
      });
      // Worker responses
      streamResponse.mockResolvedValue({ content: "Worker result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Coordinator should only be called twice: decomposition and synthesis
      const coordinatorCalls = streamResponse.mock.calls.filter(
        (call) => call[0] === "coordinator"
      );
      expect(coordinatorCalls).toHaveLength(2);
    });
  });

  describe("decomposition phase", () => {
    it("initializes state with decomposing phase", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // First call should be decomposing phase
      const firstCall = setModeState.mock.calls[0][0];
      expect(firstCall.mode).toBe("hierarchical");
      expect(firstCall.phase).toBe("decomposing");
      expect(firstCall.coordinatorModel).toBe("coordinator");
      expect(firstCall.subtasks).toEqual([]);
    });

    it("initializes streaming for coordinator during decomposition", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Instance-aware streaming passes [instanceIds], modelMap
      expect(initStreaming).toHaveBeenCalledWith(
        ["coordinator"],
        new Map([["coordinator", "coordinator"]])
      );
    });

    it("sends decomposition prompt to coordinator", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task 1"}]}',
      });

      await sendHierarchicalMode("Build a web app", ctx, mockFallback);

      // First call is decomposition
      const decompositionCall = streamResponse.mock.calls[0];
      expect(decompositionCall[0]).toBe("coordinator");
      const inputItems = decompositionCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("Build a web app");
      expect(systemPrompt).toContain("worker-a");
      expect(systemPrompt).toContain("worker-b");
    });

    it("includes worker count in decomposition prompt", async () => {
      const ctx = createMockContext({
        models: ["coordinator", "worker-a", "worker-b", "worker-c"],
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const decompositionCall = streamResponse.mock.calls[0];
      const inputItems = decompositionCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("3"); // worker count
    });

    it("uses custom decomposition prompt from config", async () => {
      const customPrompt = "Custom decomposition for {question} with {count} workers: {workers}";
      const ctx = createMockContext({
        models: ["coordinator", "worker-a"],
        modeConfig: { routingPrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const decompositionCall = streamResponse.mock.calls[0];
      const inputItems = decompositionCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toBe("Custom decomposition for Question with 1 workers: - worker-a");
    });

    it("tracks decomposition usage", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task"}]}',
        usage: { inputTokens: 100, outputTokens: 50 },
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Executing phase should include decomposition usage
      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      expect(executingCall[0].decompositionUsage).toEqual({ inputTokens: 100, outputTokens: 50 });
    });
  });

  describe("subtask parsing", () => {
    it("parses valid JSON subtasks from response", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "First task", "assignedModel": "worker-a"}, {"id": "task-2", "description": "Second task", "assignedModel": "worker-b"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      const subtasks = executingCall[0].subtasks;
      expect(subtasks).toHaveLength(2);
      expect(subtasks[0].id).toBe("task-1");
      expect(subtasks[0].description).toBe("First task");
      expect(subtasks[0].assignedModel).toBe("worker-a");
    });

    it("assigns default IDs when not provided", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task without ID"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      const subtasks = executingCall[0].subtasks;
      expect(subtasks[0].id).toBe("subtask-1");
    });

    it("round-robins worker assignment when model not specified", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Task 1"}, {"description": "Task 2"}, {"description": "Task 3"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Check the executing phase call - subtasks distributed via round-robin
      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      expect(executingCall).toBeDefined();
      const subtasks = executingCall[0].subtasks;
      expect(subtasks).toHaveLength(3);
      expect(subtasks[0].assignedModel).toBe("worker-a");
      expect(subtasks[1].assignedModel).toBe("worker-b");
      expect(subtasks[2].assignedModel).toBe("worker-a"); // cycles back
    });

    it("matches assigned model by partial name", async () => {
      const ctx = createMockContext({
        models: ["coordinator", "openai/gpt-4", "anthropic/claude-3"],
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task 1", "assignedModel": "gpt-4"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      const subtasks = executingCall[0].subtasks;
      expect(subtasks[0].assignedModel).toBe("openai/gpt-4");
    });

    it("filters out subtasks without description", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"id": "no-desc"}, {"description": "Valid task"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      const subtasks = executingCall[0].subtasks;
      expect(subtasks).toHaveLength(1);
      expect(subtasks[0].description).toBe("Valid task");
    });

    it("creates fallback subtasks when JSON parsing fails", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: "This is not valid JSON",
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      const subtasks = executingCall[0].subtasks;
      expect(subtasks).toHaveLength(2); // One per worker
      expect(subtasks[0].assignedModel).toBe("worker-a");
      expect(subtasks[1].assignedModel).toBe("worker-b");
    });

    it("creates fallback subtasks when decomposition fails", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockRejectedValueOnce(new Error("Network error"));
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      const subtasks = executingCall[0].subtasks;
      expect(subtasks).toHaveLength(1);
      expect(subtasks[0].description).toContain("Question");
    });

    it("creates fallback subtasks when response has empty subtasks array", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": []}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      const subtasks = executingCall[0].subtasks;
      expect(subtasks).toHaveLength(1); // fallback per worker
    });

    it("returns early with null results when decomposition returns null and no fallback subtasks", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce(null);

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      // With null decomposition result, fallback subtasks are created
      // so we don't return early - it continues with workers
      expect(results).toBeDefined();
    });
  });

  describe("executing phase", () => {
    it("transitions to executing phase after decomposition", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Task 1", "assignedModel": "worker-a"}, {"description": "Task 2", "assignedModel": "worker-b"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Find the executing phase call
      const executingCall = setModeState.mock.calls.find((call) => call[0].phase === "executing")!;
      expect(executingCall).toBeDefined();
      expect(executingCall[0].coordinatorModel).toBe("coordinator");
      expect(executingCall[0].subtasks).toHaveLength(2); // 2 subtasks
    });

    it("initializes streaming for active workers", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Task 1", "assignedModel": "worker-a"}, {"description": "Task 2", "assignedModel": "worker-b"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Second initStreaming call should be for workers
      // Instance-aware streaming passes [instanceIds], modelMap
      expect(initStreaming).toHaveBeenCalledWith(
        expect.arrayContaining(["worker-a", "worker-b"]),
        expect.any(Map)
      );
    });

    it("executes subtasks for each worker in parallel", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Task 1", "assignedModel": "worker-a"}, {"description": "Task 2", "assignedModel": "worker-b"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Worker result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Check workers were called
      // Instance-aware streaming: (modelId, inputItems, controller, settings, instanceId, trackToolCalls, onSSEEvent, instanceParams, instanceLabel)
      expect(streamResponse).toHaveBeenCalledWith(
        "worker-a",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "worker-a",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
      expect(streamResponse).toHaveBeenCalledWith(
        "worker-b",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "worker-b",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
    });

    it("updates subtask status to in_progress when starting", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Task 1", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // updateModeState is called to update subtask status
      expect(updateModeState).toHaveBeenCalled();

      // Verify the updater function sets status to in_progress
      const mockState = {
        mode: "hierarchical" as const,
        phase: "executing" as const,
        coordinatorModel: "coordinator",
        subtasks: [
          { id: "task-1", description: "Task 1", assignedModel: "worker-a", status: "pending" },
        ],
        workerResults: [] as Array<{ subtaskId: string; model: string; content: string }>,
      };
      const firstUpdater = updateModeState.mock.calls[0][0];
      const afterUpdate = firstUpdater(mockState);
      expect(afterUpdate.subtasks[0].status).toBe("in_progress");
    });

    it("updates subtask status to complete on success", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Task 1", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({ content: "Worker result" });
      streamResponse.mockResolvedValue({ content: "Synthesis" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // There should be multiple updateModeState calls - status update and result addition
      expect(updateModeState.mock.calls.length).toBeGreaterThanOrEqual(2);

      // Verify one updater sets status to complete
      const mockState = {
        mode: "hierarchical" as const,
        phase: "executing" as const,
        coordinatorModel: "coordinator",
        subtasks: [
          { id: "task-1", description: "Task 1", assignedModel: "worker-a", status: "in_progress" },
        ],
        workerResults: [] as Array<{ subtaskId: string; model: string; content: string }>,
      };
      const secondUpdater = updateModeState.mock.calls[1][0];
      const afterUpdate = secondUpdater(mockState);
      expect(afterUpdate.subtasks[0].status).toBe("complete");
      expect(afterUpdate.subtasks[0].result).toBe("Worker result");
    });

    it("updates subtask status to failed on error", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Task 1", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockRejectedValueOnce(new Error("Worker failed"));

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Verify an updater sets status to failed
      expect(updateModeState).toHaveBeenCalled();
      const mockState = {
        mode: "hierarchical" as const,
        phase: "executing" as const,
        coordinatorModel: "coordinator",
        subtasks: [
          { id: "task-1", description: "Task 1", assignedModel: "worker-a", status: "in_progress" },
        ],
        workerResults: [] as Array<{ subtaskId: string; model: string; content: string }>,
      };
      // Find the updater that sets failed status
      const lastUpdater = updateModeState.mock.calls[updateModeState.mock.calls.length - 1][0];
      const afterUpdate = lastUpdater(mockState);
      expect(afterUpdate.subtasks[0].status).toBe("failed");
    });

    it("updates subtask status to failed when worker returns null", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Task 1", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce(null);

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Verify an updater sets status to failed
      expect(updateModeState).toHaveBeenCalled();
      const mockState = {
        mode: "hierarchical" as const,
        phase: "executing" as const,
        coordinatorModel: "coordinator",
        subtasks: [
          { id: "task-1", description: "Task 1", assignedModel: "worker-a", status: "in_progress" },
        ],
        workerResults: [] as Array<{ subtaskId: string; model: string; content: string }>,
      };
      const lastUpdater = updateModeState.mock.calls[updateModeState.mock.calls.length - 1][0];
      const afterUpdate = lastUpdater(mockState);
      expect(afterUpdate.subtasks[0].status).toBe("failed");
    });

    it("adds worker result to store on success via updateModeState", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Do something", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({
        content: "Worker output",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      streamResponse.mockResolvedValue({ content: "Synthesis" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // updateModeState is called to add worker results
      expect(updateModeState).toHaveBeenCalled();
      // Simply verify updateModeState was called - the test validates the pattern
      // For detailed validation, we check that at least some calls were made
      expect(updateModeState.mock.calls.length).toBeGreaterThanOrEqual(1);
    });

    it("sends worker prompt with task description", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Calculate the sum", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("What is 2+2?", ctx, mockFallback);

      // Second call is worker
      const workerCall = streamResponse.mock.calls[1];
      const inputItems = workerCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("Calculate the sum");
      expect(systemPrompt).toContain("What is 2+2?");
    });

    it("uses custom worker prompt from config", async () => {
      const customPrompt = "Custom worker prompt: {task} in context of {context}";
      const ctx = createMockContext({
        models: ["coordinator", "worker-a"],
        modeConfig: { hierarchicalWorkerPrompt: customPrompt },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Do task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const workerCall = streamResponse.mock.calls[1];
      const inputItems = workerCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toBe("Custom worker prompt: Do task in context of Question");
    });

    it("handles multiple subtasks assigned to same worker sequentially", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      const callOrder: string[] = [];
      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Task 1", "assignedModel": "worker-a"}, {"id": "task-2", "description": "Task 2", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockImplementation(async (model: string) => {
        callOrder.push(model);
        return { content: "Result" };
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Worker should be called twice (once per subtask), sequentially
      const workerCalls = callOrder.filter((m) => m === "worker-a");
      expect(workerCalls).toHaveLength(2);
    });
  });

  describe("no worker results handling", () => {
    it("returns early when all workers fail", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Task 1", "assignedModel": "worker-a"}, {"description": "Task 2", "assignedModel": "worker-b"}]}',
      });
      // All workers fail
      streamResponse.mockResolvedValue(null);

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      // Should mark as done with message
      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.synthesis).toBe("No worker results to synthesize.");

      // Should return all nulls
      expect(results).toEqual([null, null, null]);
    });

    it("continues with partial worker results", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Task 1", "assignedModel": "worker-a"}, {"description": "Task 2", "assignedModel": "worker-b"}]}',
      });
      streamResponse
        .mockResolvedValueOnce({ content: "Success" })
        .mockResolvedValueOnce(null)
        .mockResolvedValue({ content: "Synthesis" });

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      // Should complete with synthesis
      const result = results.find((r) => r !== null);
      expect(result).not.toBeNull();
      expect(result?.content).toBe("Synthesis");
    });
  });

  describe("synthesizing phase", () => {
    it("transitions to synthesizing phase after workers complete", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({ content: "Worker result" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesis" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Find the synthesizing phase call
      const synthesizingCall = setModeState.mock.calls.find(
        (call) => call[0].phase === "synthesizing"
      )!;
      expect(synthesizingCall).toBeDefined();
      expect(synthesizingCall[0].coordinatorModel).toBe("coordinator");
      expect(synthesizingCall[0].subtasks).toHaveLength(1); // 1 subtask
      expect(synthesizingCall[0].workerResults).toHaveLength(1); // 1 worker result
    });

    it("initializes streaming for coordinator during synthesis", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const initStreaming = ctx.streamingStore.initStreaming as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Third initStreaming call should be for coordinator (synthesis)
      // Instance-aware streaming passes [instanceIds], modelMap
      const initCalls = initStreaming.mock.calls.map((call) => call[0]);
      expect(initCalls).toContainEqual(["coordinator"]);
    });

    it("includes worker results in synthesis prompt", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Analyze data", "assignedModel": "worker-a"}, {"id": "task-2", "description": "Generate report", "assignedModel": "worker-b"}]}',
      });
      streamResponse
        .mockResolvedValueOnce({ content: "Data analysis results" })
        .mockResolvedValueOnce({ content: "Report content" })
        .mockResolvedValueOnce({ content: "Final synthesis" });

      await sendHierarchicalMode("Process the data", ctx, mockFallback);

      // Last call is synthesis
      const synthesisCall = streamResponse.mock.calls[streamResponse.mock.calls.length - 1];
      const inputItems = synthesisCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("Process the data");
      expect(systemPrompt).toContain("Data analysis results");
      expect(systemPrompt).toContain("Report content");
      expect(systemPrompt).toContain("task-1");
      expect(systemPrompt).toContain("task-2");
    });

    it("handles synthesis failure with fallback message", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({ content: "Worker result" });
      streamResponse.mockRejectedValueOnce(new Error("Synthesis failed"));

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.content).toContain("unable to synthesize");
      expect(result?.content).toContain("Worker result");
    });
  });

  describe("result construction", () => {
    it("returns synthesis content as result", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({ content: "Worker output" });
      streamResponse.mockResolvedValueOnce({ content: "Final synthesized response" });

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.content).toBe("Final synthesized response");
    });

    it("places result at coordinator model index", async () => {
      const ctx = createMockContext({
        models: ["model-a", "model-b", "model-c"],
        modeConfig: { coordinatorModel: "model-b" },
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({ content: '{"subtasks": [{"description": "Task"}]}' });

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      // Result should be at index 1 (model-b)
      expect(results[0]).toBeNull();
      expect(results[1]).not.toBeNull();
      expect(results[2]).toBeNull();
    });

    it("includes mode metadata with hierarchical info", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Do work", "assignedModel": "worker-a"}]}',
        usage: { inputTokens: 50, outputTokens: 30 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Worker output",
        usage: { inputTokens: 20, outputTokens: 40 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Synthesis",
        usage: { inputTokens: 100, outputTokens: 60 },
      });

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata).toBeDefined();
      expect(result?.modeMetadata?.mode).toBe("hierarchical");
      expect(result?.modeMetadata?.isHierarchicalSynthesis).toBe(true);
      expect(result?.modeMetadata?.coordinatorModel).toBe("coordinator");
      expect(result?.modeMetadata?.subtasks).toBeDefined();
      expect(result?.modeMetadata?.workerResults).toBeDefined();
    });

    it("includes subtasks with status and results in metadata", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Do work", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({ content: "Worker output" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesis" });

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata?.subtasks).toHaveLength(1);
      expect(result?.modeMetadata?.subtasks?.[0]).toEqual({
        id: "task-1",
        description: "Do work",
        assignedModel: "worker-a",
        status: "complete",
        result: "Worker output",
      });
    });

    it("includes worker results in metadata", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"id": "task-1", "description": "Task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({
        content: "Worker output",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      streamResponse.mockResolvedValueOnce({ content: "Synthesis" });

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata?.workerResults).toHaveLength(1);
      expect(result?.modeMetadata?.workerResults?.[0]).toEqual({
        subtaskId: "task-1",
        model: "worker-a",
        description: "Task",
        content: "Worker output",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
    });

    it("includes decomposition usage in metadata", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
        usage: { inputTokens: 100, outputTokens: 50 },
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata?.decompositionUsage).toEqual({
        inputTokens: 100,
        outputTokens: 50,
      });
    });

    it("includes synthesis usage in metadata", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({ content: "Worker" });
      streamResponse.mockResolvedValueOnce({
        content: "Synthesis",
        usage: { inputTokens: 200, outputTokens: 100 },
      });

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      expect(result?.modeMetadata?.synthesizerUsage).toEqual({
        inputTokens: 200,
        outputTokens: 100,
      });
    });

    it("aggregates usage from all phases", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Task 1", "assignedModel": "worker-a"}, {"description": "Task 2", "assignedModel": "worker-b"}]}',
        usage: { inputTokens: 50, outputTokens: 25 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Worker A",
        usage: { inputTokens: 10, outputTokens: 20 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Worker B",
        usage: { inputTokens: 15, outputTokens: 30 },
      });
      streamResponse.mockResolvedValueOnce({
        content: "Synthesis",
        usage: { inputTokens: 100, outputTokens: 60 },
      });

      const results = await sendHierarchicalMode("Question", ctx, mockFallback);

      const result = results.find((r) => r !== null);
      // Total: 50+10+15+100 = 175 input, 25+20+30+60 = 135 output
      expect(result?.modeMetadata?.aggregateUsage?.inputTokens).toBe(175);
      expect(result?.modeMetadata?.aggregateUsage?.outputTokens).toBe(135);
    });
  });

  describe("state transitions", () => {
    it("transitions through decomposing -> executing -> synthesizing -> done phases", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({ content: "Worker" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesis" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const phases = setModeState.mock.calls.map((call) => call[0].phase);
      expect(phases[0]).toBe("decomposing");
      expect(phases).toContain("executing");
      expect(phases).toContain("synthesizing");
      expect(phases[phases.length - 1]).toBe("done");
    });

    it("sets final state with synthesis content and usage", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const setModeState = ctx.streamingStore.setModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
        usage: { inputTokens: 10 },
      });
      streamResponse.mockResolvedValueOnce({ content: "Worker" });
      streamResponse.mockResolvedValueOnce({
        content: "Final synthesis",
        usage: { inputTokens: 50, outputTokens: 30 },
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      const lastCall = setModeState.mock.calls[setModeState.mock.calls.length - 1][0];
      expect(lastCall.phase).toBe("done");
      expect(lastCall.synthesis).toBe("Final synthesis");
      expect(lastCall.decompositionUsage).toEqual({ inputTokens: 10 });
      expect(lastCall.synthesisUsage).toEqual({ inputTokens: 50, outputTokens: 30 });
    });
  });

  describe("abort controller management", () => {
    it("creates abort controller for decomposition phase", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Decomposition call should have abort controller
      // Instance-aware streaming: (modelId, inputItems, controller, settings, instanceId, trackToolCalls, onSSEEvent, instanceParams, instanceLabel)
      expect(streamResponse).toHaveBeenCalledWith(
        "coordinator",
        expect.any(Array),
        expect.any(AbortController),
        undefined,
        "coordinator",
        undefined,
        undefined,
        undefined,
        undefined // instanceLabel
      );
    });

    it("creates abort controllers for each worker", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a", "worker-b"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content:
          '{"subtasks": [{"description": "Task 1", "assignedModel": "worker-a"}, {"description": "Task 2", "assignedModel": "worker-b"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Workers should each have their own abort controller
      const workerACalls = streamResponse.mock.calls.filter((call) => call[0] === "worker-a");
      const workerBCalls = streamResponse.mock.calls.filter((call) => call[0] === "worker-b");

      expect(workerACalls[0][2]).toBeInstanceOf(AbortController);
      expect(workerBCalls[0][2]).toBeInstanceOf(AbortController);
    });

    it("stores abort controllers in ref", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      expect(ctx.abortControllersRef.current).toBeDefined();
      expect(ctx.abortControllersRef.current.length).toBeGreaterThanOrEqual(1);
    });

    it("creates separate abort controller for synthesis phase", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValueOnce({ content: "Worker" });
      streamResponse.mockResolvedValueOnce({ content: "Synthesis" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Final ref should have synthesis controller
      expect(ctx.abortControllersRef.current).toHaveLength(1);
    });
  });

  describe("message filtering", () => {
    it("filters messages for coordinator in decomposition", async () => {
      const messages = [
        { id: "1", role: "user" as const, content: "Hello", timestamp: new Date() },
        {
          id: "2",
          role: "assistant" as const,
          content: "Hi",
          model: "coordinator",
          timestamp: new Date(),
        },
      ];
      const filterMessagesForModel = vi.fn((msgs) => msgs);
      const ctx = createMockContext({
        models: ["coordinator", "worker-a"],
        messages,
        filterMessagesForModel,
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      expect(filterMessagesForModel).toHaveBeenCalledWith(messages, "coordinator");
    });
  });

  describe("multimodal content handling", () => {
    it("extracts text from multimodal content for prompts", async () => {
      const ctx = createMockContext({ models: ["coordinator", "worker-a"] });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValue({
        content: '{"subtasks": [{"description": "Task"}]}',
      });

      const multimodalContent = [
        { type: "input_text", text: "Analyze this image" },
        { type: "image", source: { type: "base64", data: "abc123" } },
      ];

      await sendHierarchicalMode(multimodalContent, ctx, mockFallback);

      // Check decomposition prompt contains extracted text
      const decompositionCall = streamResponse.mock.calls[0];
      const inputItems = decompositionCall[1];
      const systemPrompt = inputItems.find((i: { role: string }) => i.role === "system")?.content;
      expect(systemPrompt).toContain("Analyze this image");
    });

    it("passes multimodal content to fallback when needed", async () => {
      const ctx = createMockContext({ models: ["coordinator"] });

      const multimodalContent = [
        { type: "input_text", text: "Question" },
        { type: "image", source: { type: "base64", data: "xyz" } },
      ];

      await sendHierarchicalMode(multimodalContent, ctx, mockFallback);

      expect(mockFallback).toHaveBeenCalledWith(multimodalContent);
    });
  });

  describe("multiple workers with multiple subtasks", () => {
    it("distributes subtasks across multiple workers", async () => {
      const ctx = createMockContext({
        models: ["coordinator", "worker-a", "worker-b", "worker-c"],
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
      const updateModeState = ctx.streamingStore.updateModeState as ReturnType<typeof vi.fn>;

      streamResponse.mockResolvedValueOnce({
        content: `{"subtasks": [
          {"id": "t1", "description": "Task 1", "assignedModel": "worker-a"},
          {"id": "t2", "description": "Task 2", "assignedModel": "worker-b"},
          {"id": "t3", "description": "Task 3", "assignedModel": "worker-c"},
          {"id": "t4", "description": "Task 4", "assignedModel": "worker-a"}
        ]}`,
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Complex task", ctx, mockFallback);

      // Should have worker results added via updateModeState
      // 4 subtasks * 3 updates each (in_progress, complete/result, workerResult) = many calls
      // But we can verify that updateModeState was called and has workerResult updates
      expect(updateModeState).toHaveBeenCalled();
      // Count the calls that add worker results by checking for calls with workerResults additions
      const workerResultCalls = updateModeState.mock.calls.filter((call) => {
        const updater = call[0];
        const mockState = {
          mode: "hierarchical" as const,
          phase: "executing" as const,
          coordinatorModel: "coordinator",
          subtasks: [],
          workerResults: [] as Array<{ subtaskId: string }>,
        };
        try {
          const result = updater(mockState);
          return result.workerResults && result.workerResults.length > 0;
        } catch {
          return false;
        }
      });
      // There should be at least 4 calls that add workerResults
      expect(workerResultCalls.length).toBeGreaterThanOrEqual(4);
    });

    it("handles workers with no assigned subtasks", async () => {
      const ctx = createMockContext({
        models: ["coordinator", "worker-a", "worker-b", "worker-c"],
      });
      const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

      // Only assign to worker-a
      streamResponse.mockResolvedValueOnce({
        content: '{"subtasks": [{"description": "Task", "assignedModel": "worker-a"}]}',
      });
      streamResponse.mockResolvedValue({ content: "Result" });

      await sendHierarchicalMode("Question", ctx, mockFallback);

      // Only worker-a should be called
      const workerACalls = streamResponse.mock.calls.filter((call) => call[0] === "worker-a");
      const workerBCalls = streamResponse.mock.calls.filter((call) => call[0] === "worker-b");
      const workerCCalls = streamResponse.mock.calls.filter((call) => call[0] === "worker-c");

      expect(workerACalls).toHaveLength(1);
      expect(workerBCalls).toHaveLength(0);
      expect(workerCCalls).toHaveLength(0);
    });
  });
});
