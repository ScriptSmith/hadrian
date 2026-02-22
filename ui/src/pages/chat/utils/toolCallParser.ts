/**
 * Tool Call Parser - Types and utilities for parsing tool calls from SSE events
 *
 * This module provides the foundation for client-side tool execution by:
 * 1. Defining types for tool call events from the backend SSE stream
 * 2. Tracking tool call state during streaming
 * 3. Parsing SSE events to detect and extract tool calls
 *
 * ## Backend Event Flow
 *
 * When the model requests a tool call (e.g., file_search), the backend emits:
 * 1. `response.output_item.added` - Tool call initiated (contains id, name)
 * 2. `response.function_call_arguments.delta` - Argument chunks (streaming)
 * 3. `response.function_call_arguments.done` - Final arguments (JSON string)
 * 4. `response.output_item.done` - Tool call complete
 *
 * ## Usage
 *
 * ```typescript
 * const tracker = createToolCallTracker();
 *
 * // In SSE event handler:
 * const result = parseToolCallFromEvent(event, tracker);
 * if (result?.type === "tool_call_complete") {
 *   // Execute tool and send result back
 *   const toolResult = await executeFileSearch(result.toolCall);
 * }
 * ```
 */

import type { ToolCall, ToolCallType, ToolCallStatus } from "@/components/ToolCallIndicator";

// Re-export for convenience
export type { ToolCall, ToolCallType, ToolCallStatus };

/**
 * SSE event types emitted by the backend for function calls
 */
export type FunctionCallEventType =
  | "response.output_item.added"
  | "response.function_call_arguments.delta"
  | "response.function_call_arguments.done"
  | "response.output_item.done";

/**
 * Base structure for all SSE events from the backend
 */
export interface BaseSSEEvent {
  type: string;
  [key: string]: unknown;
}

/**
 * Event: response.output_item.added
 * Emitted when a new output item (message or function_call) is added
 */
export interface OutputItemAddedEvent extends BaseSSEEvent {
  type: "response.output_item.added";
  output_index: number;
  item: {
    type: "function_call" | "message";
    id: string;
    call_id?: string; // Present for function_call
    name?: string; // Present for function_call
    role?: string; // Present for message
    status?: string;
  };
}

/**
 * Event: response.function_call_arguments.delta
 * Emitted during streaming of function call arguments
 */
export interface FunctionCallArgumentsDeltaEvent extends BaseSSEEvent {
  type: "response.function_call_arguments.delta";
  item_id: string;
  output_index: number;
  delta: string;
}

/**
 * Event: response.function_call_arguments.done
 * Emitted when function call arguments are complete
 */
export interface FunctionCallArgumentsDoneEvent extends BaseSSEEvent {
  type: "response.function_call_arguments.done";
  item_id: string;
  output_index: number;
  arguments: string; // JSON string
}

/**
 * Event: response.output_item.done
 * Emitted when an output item is complete
 */
export interface OutputItemDoneEvent extends BaseSSEEvent {
  type: "response.output_item.done";
  output_index: number;
  item: {
    type: "function_call" | "message";
    id: string;
    call_id?: string;
    name?: string;
    arguments?: string;
    status: string;
    role?: string;
    content?: Array<{ type: string; text?: string }>;
  };
}

/**
 * Union type for all function call related events
 */
export type FunctionCallEvent =
  | OutputItemAddedEvent
  | FunctionCallArgumentsDeltaEvent
  | FunctionCallArgumentsDoneEvent
  | OutputItemDoneEvent;

/**
 * Parsed file_search tool call with typed arguments
 */
export interface FileSearchToolCall {
  id: string;
  callId: string;
  name: "file_search";
  status: ToolCallStatus;
  arguments: FileSearchArguments;
}

/**
 * Arguments for file_search tool call
 */
export interface FileSearchArguments {
  query: string;
  vector_store_ids?: string[];
  max_num_results?: number;
  score_threshold?: number;
}

/**
 * Generic parsed tool call (for non-file_search tools)
 */
export interface GenericToolCall {
  id: string;
  callId: string;
  name: string;
  status: ToolCallStatus;
  arguments: Record<string, unknown>;
}

/**
 * Union of all parsed tool call types
 */
export type ParsedToolCall = FileSearchToolCall | GenericToolCall;

/**
 * State for tracking a tool call during streaming
 */
export interface ToolCallState {
  /** Unique ID for this tool call (fc_xxx format) */
  id: string;
  /** Call ID from the provider (toolu_xxx format) */
  callId: string;
  /** Tool/function name */
  name: string;
  /** Output index in the response */
  outputIndex: number;
  /** Accumulated arguments (JSON string, built from deltas) */
  argumentsBuffer: string;
  /** Current status */
  status: ToolCallStatus;
  /** Parsed arguments (set when done) */
  parsedArguments?: Record<string, unknown>;
  /** Error message if status is "failed" */
  error?: string;
}

/**
 * Tracker for managing multiple tool calls during a streaming session
 */
export interface ToolCallTracker {
  /** Map of tool call ID to state */
  toolCalls: Map<string, ToolCallState>;
  /** Get all tool calls as array (for UI rendering) */
  getToolCalls(): ToolCall[];
  /** Check if any tool calls are pending/executing */
  hasPendingToolCalls(): boolean;
  /** Get completed tool calls ready for execution */
  getCompletedToolCalls(): ParsedToolCall[];
  /** Clear all tracked tool calls */
  clear(): void;
}

/**
 * Result from parsing an SSE event
 */
export type ParseResult =
  | { type: "tool_call_added"; toolCall: ToolCallState }
  | { type: "tool_call_arguments_delta"; id: string; delta: string }
  | { type: "tool_call_arguments_done"; id: string; arguments: string }
  | { type: "tool_call_complete"; toolCall: ParsedToolCall }
  | { type: "ignored" }
  | { type: "error"; message: string };

/**
 * Create a new tool call tracker
 */
export function createToolCallTracker(): ToolCallTracker {
  const toolCalls = new Map<string, ToolCallState>();

  return {
    toolCalls,

    getToolCalls(): ToolCall[] {
      return Array.from(toolCalls.values()).map((tc) => ({
        id: tc.id,
        type: mapToolNameToType(tc.name),
        name: tc.name,
        status: tc.status,
      }));
    },

    hasPendingToolCalls(): boolean {
      return Array.from(toolCalls.values()).some(
        (tc) => tc.status === "pending" || tc.status === "executing"
      );
    },

    getCompletedToolCalls(): ParsedToolCall[] {
      return Array.from(toolCalls.values())
        .filter((tc) => tc.status === "completed" && tc.parsedArguments)
        .map((tc) => createParsedToolCall(tc));
    },

    clear(): void {
      toolCalls.clear();
    },
  };
}

/**
 * Map tool function name to ToolCallType for UI display
 */
function mapToolNameToType(name: string): ToolCallType {
  switch (name) {
    case "file_search":
      return "file_search";
    case "web_search":
      return "web_search";
    case "code_interpreter":
      return "code_interpreter";
    case "js_code_interpreter":
      return "js_code_interpreter";
    default:
      return "function";
  }
}

/**
 * Create a ParsedToolCall from ToolCallState
 */
function createParsedToolCall(state: ToolCallState): ParsedToolCall {
  const base = {
    id: state.id,
    callId: state.callId,
    name: state.name,
    status: state.status,
  };

  if (state.name === "file_search") {
    // Cast through unknown since parsedArguments is Record<string, unknown>
    // but we know file_search tool calls have FileSearchArguments structure
    const args = (state.parsedArguments ?? {}) as unknown as FileSearchArguments;
    return {
      ...base,
      name: "file_search" as const,
      arguments: args,
    };
  }

  return {
    ...base,
    arguments: state.parsedArguments ?? {},
  };
}

/**
 * Check if an event is a function call related event
 */
export function isFunctionCallEvent(event: BaseSSEEvent): event is FunctionCallEvent {
  return (
    event.type === "response.output_item.added" ||
    event.type === "response.function_call_arguments.delta" ||
    event.type === "response.function_call_arguments.done" ||
    event.type === "response.output_item.done"
  );
}

/**
 * Parse an SSE event and update the tool call tracker
 *
 * @param event - The SSE event to parse
 * @param tracker - The tool call tracker to update
 * @returns ParseResult indicating what happened
 */
export function parseToolCallFromEvent(event: BaseSSEEvent, tracker: ToolCallTracker): ParseResult {
  if (!isFunctionCallEvent(event)) {
    return { type: "ignored" };
  }

  switch (event.type) {
    case "response.output_item.added": {
      const addedEvent = event as OutputItemAddedEvent;

      // Only track function_call items, not messages
      if (addedEvent.item.type !== "function_call") {
        return { type: "ignored" };
      }

      // Use call_id as the canonical identifier - it's required for matching
      // function_call_output back to the original call, so it's always present
      const callId = addedEvent.item.call_id ?? addedEvent.item.id;
      const state: ToolCallState = {
        id: callId,
        callId: callId,
        name: addedEvent.item.name ?? "unknown",
        outputIndex: addedEvent.output_index,
        argumentsBuffer: "",
        status: "pending",
      };

      tracker.toolCalls.set(callId, state);
      return { type: "tool_call_added", toolCall: state };
    }

    case "response.function_call_arguments.delta": {
      const deltaEvent = event as FunctionCallArgumentsDeltaEvent;
      const state = tracker.toolCalls.get(deltaEvent.item_id);

      if (!state) {
        return {
          type: "error",
          message: `Received arguments delta for unknown tool call: ${deltaEvent.item_id}`,
        };
      }

      // Update status to executing on first delta
      if (state.status === "pending") {
        state.status = "executing";
      }

      // Append to arguments buffer
      state.argumentsBuffer += deltaEvent.delta;
      return { type: "tool_call_arguments_delta", id: state.id, delta: deltaEvent.delta };
    }

    case "response.function_call_arguments.done": {
      const doneEvent = event as FunctionCallArgumentsDoneEvent;
      const state = tracker.toolCalls.get(doneEvent.item_id);

      if (!state) {
        return {
          type: "error",
          message: `Received arguments done for unknown tool call: ${doneEvent.item_id}`,
        };
      }

      // Use the final arguments from the event (more reliable than buffer)
      state.argumentsBuffer = doneEvent.arguments;

      // Parse the arguments JSON
      try {
        state.parsedArguments = JSON.parse(doneEvent.arguments) as Record<string, unknown>;
      } catch {
        return {
          type: "error",
          message: `Failed to parse arguments for tool call ${state.id}: ${doneEvent.arguments}`,
        };
      }

      return { type: "tool_call_arguments_done", id: state.id, arguments: doneEvent.arguments };
    }

    case "response.output_item.done": {
      const itemDoneEvent = event as OutputItemDoneEvent;

      // Only process function_call items
      if (itemDoneEvent.item.type !== "function_call") {
        return { type: "ignored" };
      }

      // Use call_id as the canonical identifier
      const callId = itemDoneEvent.item.call_id ?? itemDoneEvent.item.id;
      const state = tracker.toolCalls.get(callId);

      if (!state) {
        // Item might have been created without output_item.added (edge case)
        // Create it now from the done event
        const newState: ToolCallState = {
          id: callId,
          callId: callId,
          name: itemDoneEvent.item.name ?? "unknown",
          outputIndex: itemDoneEvent.output_index,
          argumentsBuffer: itemDoneEvent.item.arguments ?? "",
          status: "completed",
        };

        // Parse arguments
        if (itemDoneEvent.item.arguments) {
          try {
            newState.parsedArguments = JSON.parse(itemDoneEvent.item.arguments) as Record<
              string,
              unknown
            >;
          } catch {
            return {
              type: "error",
              message: `Failed to parse arguments from output_item.done: ${itemDoneEvent.item.arguments}`,
            };
          }
        }

        tracker.toolCalls.set(callId, newState);
        return { type: "tool_call_complete", toolCall: createParsedToolCall(newState) };
      }

      // Update existing state
      state.status = "completed";

      // If we don't have parsed arguments yet, try from the done event
      if (!state.parsedArguments && itemDoneEvent.item.arguments) {
        try {
          state.parsedArguments = JSON.parse(itemDoneEvent.item.arguments) as Record<
            string,
            unknown
          >;
        } catch {
          return {
            type: "error",
            message: `Failed to parse arguments from output_item.done: ${itemDoneEvent.item.arguments}`,
          };
        }
      }

      return { type: "tool_call_complete", toolCall: createParsedToolCall(state) };
    }

    default:
      return { type: "ignored" };
  }
}

/**
 * Type guard to check if a parsed tool call is a file_search call
 */
export function isFileSearchToolCall(toolCall: ParsedToolCall): toolCall is FileSearchToolCall {
  return toolCall.name === "file_search";
}

/**
 * Extract file_search query from a tool call
 * Returns null if not a file_search call or query is missing
 */
export function extractFileSearchQuery(toolCall: ParsedToolCall): string | null {
  if (!isFileSearchToolCall(toolCall)) {
    return null;
  }
  return toolCall.arguments.query ?? null;
}
