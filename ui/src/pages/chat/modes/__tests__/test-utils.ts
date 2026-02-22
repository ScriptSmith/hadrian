/**
 * Shared test utilities for mode handler tests.
 *
 * This module consolidates common test setup code that was duplicated
 * across all 14+ mode test files.
 */

import { vi, type Mock } from "vitest";
import type { ModeContext, ModeResult } from "../types";

/**
 * Creates a mock abort controllers ref.
 * This is identical across all mode tests.
 */
export function createMockAbortControllersRef() {
  return { current: [] as AbortController[] };
}

/**
 * Streaming store method signatures used by mode handlers.
 * All modes now use the unified setModeState/updateModeState actions.
 */
export interface StreamingStoreMethods {
  // Common streaming operations
  initStreaming?: ReturnType<typeof vi.fn>;

  // Unified mode state (discriminated union approach)
  setModeState?: ReturnType<typeof vi.fn>;
  updateModeState?: ReturnType<typeof vi.fn>;
}

/**
 * All streaming store method names.
 */
const ALL_STREAMING_STORE_METHODS: (keyof StreamingStoreMethods)[] = [
  "initStreaming",
  "setModeState",
  "updateModeState",
];

/**
 * Creates a mock streaming store with all methods mocked.
 * All methods are automatically mocked with vi.fn().
 */
export function createMockStreamingStore(): StreamingStoreMethods {
  const store: Partial<StreamingStoreMethods> = {};

  for (const method of ALL_STREAMING_STORE_METHODS) {
    store[method] = vi.fn();
  }

  return store as StreamingStoreMethods;
}

/**
 * Creates a mock ModeContext with sensible defaults.
 * All streaming store methods are automatically mocked.
 *
 * @param overrides - Override any context fields
 * @returns A mock ModeContext suitable for testing
 */
export function createMockContext(
  overrides: Partial<ModeContext> = {}
): ModeContext & { streamingStore: StreamingStoreMethods } {
  const streamingStore = createMockStreamingStore();
  const abortControllersRef = createMockAbortControllersRef();

  return {
    models: ["model-a", "model-b", "model-c"],
    messages: [],
    settings: undefined,
    modeConfig: undefined,
    token: "test-token",
    streamingStore: streamingStore as unknown as ModeContext["streamingStore"],
    abortControllersRef: abortControllersRef as unknown as ModeContext["abortControllersRef"],
    streamResponse: vi.fn(),
    filterMessagesForModel: vi.fn((messages) => messages),
    ...overrides,
  } as ModeContext & { streamingStore: StreamingStoreMethods };
}

/**
 * Type for the fallback function passed to mode handlers.
 */
export type FallbackFn = (apiContent: string | unknown[]) => Promise<(ModeResult | null)[]>;

/**
 * Creates a mock fallback function that returns a standard response.
 */
export function createMockFallback(): Mock<FallbackFn> {
  return vi.fn<FallbackFn>().mockResolvedValue([{ content: "fallback response" }]);
}

/**
 * Standard multimodal content for testing.
 */
export const testMultimodalContent = [
  { type: "input_text", text: "Describe this image" },
  { type: "image", source: { type: "base64", data: "abc123" } },
];

/**
 * Creates standard test messages for message filtering tests.
 */
export function createTestMessages() {
  return [
    { id: "1", role: "user" as const, content: "Hello", timestamp: new Date() },
    {
      id: "2",
      role: "assistant" as const,
      content: "Hi",
      model: "model-a",
      timestamp: new Date(),
    },
  ];
}

/**
 * Helper to extract the streamResponse mock from a context.
 */
export function getStreamResponseMock(ctx: ModeContext) {
  return ctx.streamResponse as ReturnType<typeof vi.fn>;
}

/**
 * Helper to verify that a mode correctly falls back for insufficient models.
 *
 * @param sendMode - The mode function to test
 * @param minModels - Minimum models required (will test with minModels-1)
 */
export async function testFallbackBehavior(
  sendMode: (
    content: unknown,
    ctx: ModeContext,
    fallback: ReturnType<typeof vi.fn>
  ) => Promise<unknown>,
  minModels: number
) {
  const models = Array.from({ length: minModels - 1 }, (_, i) => `model-${i}`);
  const ctx = createMockContext({ models });
  const fallback = createMockFallback();

  await sendMode("Hello", ctx, fallback);

  return {
    fallbackCalled: fallback.mock.calls.length > 0,
    streamResponseCalled: (ctx.streamResponse as ReturnType<typeof vi.fn>).mock.calls.length > 0,
    ctx,
    fallback,
  };
}

/**
 * Helper to set up a sequence of mock responses for streamResponse.
 */
export function setupStreamResponses(
  ctx: ModeContext,
  responses: Array<{ content: string; usage?: object } | null>
) {
  const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;

  for (const response of responses) {
    streamResponse.mockResolvedValueOnce(response);
  }
}

/**
 * Common assertions for abort controller tests.
 */
export function assertAbortControllerPassed(ctx: ModeContext, callIndex: number): AbortController {
  const streamResponse = ctx.streamResponse as ReturnType<typeof vi.fn>;
  const controller = streamResponse.mock.calls[callIndex]?.[2];
  if (!(controller instanceof AbortController)) {
    throw new Error(`Call ${callIndex} did not receive an AbortController`);
  }
  return controller;
}

/**
 * Common assertions for checking that message filtering was called.
 */
export function assertMessagesFiltered(ctx: ModeContext, model: string): void {
  const filterFn = ctx.filterMessagesForModel as ReturnType<typeof vi.fn>;
  const wasCalledWithModel = filterFn.mock.calls.some((call) => call[1] === model);
  if (!wasCalledWithModel) {
    throw new Error(`filterMessagesForModel was not called for model: ${model}`);
  }
}

/**
 * Helper to assert that a value is defined and return it with narrowed type.
 * This is useful after Array.find() which returns T | undefined.
 */
export function assertDefined<T>(value: T | undefined, message?: string): T {
  if (value === undefined) {
    throw new Error(message ?? "Expected value to be defined");
  }
  return value;
}
