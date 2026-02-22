import { create } from "zustand";
import { useShallow } from "zustand/react/shallow";

import type { MessageDebugInfo, DebugRound, DebugSSEEvent } from "@/components/chat-types";

/**
 * Debug Store - Captures Raw Message Exchanges for Debugging
 *
 * ## Purpose
 *
 * This store captures detailed information about the request/response cycle
 * during chat interactions, primarily for debugging multi-turn tool execution loops.
 *
 * ## Data Captured
 *
 * For each round of tool execution:
 * - Input items sent to the API
 * - Full request body
 * - Response output array
 * - Tool calls detected
 * - Tool execution results
 * - Continuation items sent back
 * - Raw SSE events (optional, for detailed debugging)
 *
 * ## Usage
 *
 * The debug data is keyed by message ID and model, since a single user message
 * may result in multiple model responses (in multi-model modes).
 *
 * ```typescript
 * // Start capturing debug info for a message
 * debugStore.startDebugCapture(messageId, model);
 *
 * // Add a round of debug data
 * debugStore.addDebugRound(messageId, model, round);
 *
 * // Complete the capture
 * debugStore.completeDebugCapture(messageId, model, success, error);
 *
 * // Retrieve debug info for display
 * const debugInfo = debugStore.getDebugInfo(messageId, model);
 * ```
 *
 * ## Memory Management
 *
 * Debug data is stored in memory and cleared when:
 * - The conversation is cleared
 * - Explicitly cleared via clearAllDebugInfo()
 * - The session ends (not persisted to localStorage)
 */

/** Configuration for debug capture */
export interface DebugCaptureConfig {
  /** Whether to capture raw SSE events (increases memory usage) */
  captureSSEEvents: boolean;
  /** Maximum number of SSE events to capture per round (to prevent memory issues) */
  maxSSEEventsPerRound: number;
}

const DEFAULT_DEBUG_CONFIG: DebugCaptureConfig = {
  captureSSEEvents: true,
  maxSSEEventsPerRound: 1000,
};

interface DebugState {
  /** Map from "messageId:model" to debug info */
  debugInfoMap: Map<string, MessageDebugInfo>;

  /** Currently active captures (in-progress) */
  activeCaptures: Set<string>;

  /** Debug capture configuration */
  config: DebugCaptureConfig;

  // === Actions ===

  /** Start capturing debug info for a message/model pair */
  startDebugCapture: (messageId: string, model: string) => void;

  /** Start a new round within an active capture */
  startDebugRound: (
    messageId: string,
    model: string,
    roundNumber: number,
    inputItems: unknown[]
  ) => void;

  /** Update the current round with request body */
  setRoundRequestBody: (
    messageId: string,
    model: string,
    roundNumber: number,
    requestBody: Record<string, unknown>
  ) => void;

  /** Add an SSE event to the current round */
  addSSEEvent: (
    messageId: string,
    model: string,
    roundNumber: number,
    event: DebugSSEEvent
  ) => void;

  /** Update the current round with response output */
  setRoundResponseOutput: (
    messageId: string,
    model: string,
    roundNumber: number,
    responseOutput: unknown[]
  ) => void;

  /** Update the current round with tool calls */
  setRoundToolCalls: (
    messageId: string,
    model: string,
    roundNumber: number,
    toolCalls: DebugRound["toolCalls"]
  ) => void;

  /** Update the current round with tool results */
  setRoundToolResults: (
    messageId: string,
    model: string,
    roundNumber: number,
    toolResults: DebugRound["toolResults"]
  ) => void;

  /** Update the current round with continuation items */
  setRoundContinuationItems: (
    messageId: string,
    model: string,
    roundNumber: number,
    continuationItems: unknown[]
  ) => void;

  /** End the current round */
  endDebugRound: (messageId: string, model: string, roundNumber: number) => void;

  /** Complete the debug capture */
  completeDebugCapture: (
    messageId: string,
    model: string,
    success: boolean,
    error?: string
  ) => void;

  /** Get debug info for a message/model pair */
  getDebugInfo: (messageId: string, model: string) => MessageDebugInfo | undefined;

  /** Clear debug info for a specific message */
  clearDebugInfo: (messageId: string) => void;

  /** Clear all debug info */
  clearAllDebugInfo: () => void;

  /** Update debug configuration */
  setConfig: (config: Partial<DebugCaptureConfig>) => void;
}

/** Create the key for the debug info map */
const makeKey = (messageId: string, model: string) => `${messageId}:${model}`;

export const useDebugStore = create<DebugState>((set, get) => ({
  debugInfoMap: new Map(),
  activeCaptures: new Set(),
  config: DEFAULT_DEBUG_CONFIG,

  startDebugCapture: (messageId, model) => {
    const key = makeKey(messageId, model);
    set((state) => {
      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, {
        messageId,
        model,
        rounds: [],
        totalDuration: 0,
        success: false,
      });
      const newActive = new Set(state.activeCaptures);
      newActive.add(key);
      return { debugInfoMap: newMap, activeCaptures: newActive };
    });
  },

  startDebugRound: (messageId, model, roundNumber, inputItems) => {
    const key = makeKey(messageId, model);
    set((state) => {
      const info = state.debugInfoMap.get(key);
      if (!info) return state;

      const newRound: DebugRound = {
        round: roundNumber,
        startTime: Date.now(),
        inputItems,
        sseEvents: state.config.captureSSEEvents ? [] : undefined,
      };

      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, {
        ...info,
        rounds: [...info.rounds, newRound],
      });
      return { debugInfoMap: newMap };
    });
  },

  setRoundRequestBody: (messageId, model, roundNumber, requestBody) => {
    const key = makeKey(messageId, model);
    set((state) => {
      const info = state.debugInfoMap.get(key);
      if (!info) return state;

      const newRounds = info.rounds.map((r) =>
        r.round === roundNumber ? { ...r, requestBody } : r
      );

      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, { ...info, rounds: newRounds });
      return { debugInfoMap: newMap };
    });
  },

  addSSEEvent: (messageId, model, roundNumber, event) => {
    const key = makeKey(messageId, model);
    const state = get();
    const info = state.debugInfoMap.get(key);
    if (!info || !state.config.captureSSEEvents) return;

    const round = info.rounds.find((r) => r.round === roundNumber);
    if (!round || !round.sseEvents || round.sseEvents.length >= state.config.maxSSEEventsPerRound) {
      return;
    }

    set((state) => {
      const info = state.debugInfoMap.get(key);
      if (!info) return state;

      const newRounds = info.rounds.map((r) =>
        r.round === roundNumber && r.sseEvents ? { ...r, sseEvents: [...r.sseEvents, event] } : r
      );

      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, { ...info, rounds: newRounds });
      return { debugInfoMap: newMap };
    });
  },

  setRoundResponseOutput: (messageId, model, roundNumber, responseOutput) => {
    const key = makeKey(messageId, model);
    set((state) => {
      const info = state.debugInfoMap.get(key);
      if (!info) return state;

      const newRounds = info.rounds.map((r) =>
        r.round === roundNumber ? { ...r, responseOutput } : r
      );

      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, { ...info, rounds: newRounds });
      return { debugInfoMap: newMap };
    });
  },

  setRoundToolCalls: (messageId, model, roundNumber, toolCalls) => {
    const key = makeKey(messageId, model);
    set((state) => {
      const info = state.debugInfoMap.get(key);
      if (!info) return state;

      const newRounds = info.rounds.map((r) => (r.round === roundNumber ? { ...r, toolCalls } : r));

      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, { ...info, rounds: newRounds });
      return { debugInfoMap: newMap };
    });
  },

  setRoundToolResults: (messageId, model, roundNumber, toolResults) => {
    const key = makeKey(messageId, model);
    set((state) => {
      const info = state.debugInfoMap.get(key);
      if (!info) return state;

      const newRounds = info.rounds.map((r) =>
        r.round === roundNumber ? { ...r, toolResults } : r
      );

      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, { ...info, rounds: newRounds });
      return { debugInfoMap: newMap };
    });
  },

  setRoundContinuationItems: (messageId, model, roundNumber, continuationItems) => {
    const key = makeKey(messageId, model);
    set((state) => {
      const info = state.debugInfoMap.get(key);
      if (!info) return state;

      const newRounds = info.rounds.map((r) =>
        r.round === roundNumber ? { ...r, continuationItems } : r
      );

      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, { ...info, rounds: newRounds });
      return { debugInfoMap: newMap };
    });
  },

  endDebugRound: (messageId, model, roundNumber) => {
    const key = makeKey(messageId, model);
    set((state) => {
      const info = state.debugInfoMap.get(key);
      if (!info) return state;

      const newRounds = info.rounds.map((r) =>
        r.round === roundNumber ? { ...r, endTime: Date.now() } : r
      );

      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, { ...info, rounds: newRounds });
      return { debugInfoMap: newMap };
    });
  },

  completeDebugCapture: (messageId, model, success, error) => {
    const key = makeKey(messageId, model);
    set((state) => {
      const info = state.debugInfoMap.get(key);
      if (!info) return state;

      // Calculate total duration from all rounds
      let totalDuration = 0;
      for (const round of info.rounds) {
        if (round.endTime && round.startTime) {
          totalDuration += round.endTime - round.startTime;
        }
      }

      const newMap = new Map(state.debugInfoMap);
      newMap.set(key, {
        ...info,
        success,
        error,
        totalDuration,
      });

      const newActive = new Set(state.activeCaptures);
      newActive.delete(key);

      return { debugInfoMap: newMap, activeCaptures: newActive };
    });
  },

  getDebugInfo: (messageId, model) => {
    return get().debugInfoMap.get(makeKey(messageId, model));
  },

  clearDebugInfo: (messageId) => {
    set((state) => {
      const newMap = new Map(state.debugInfoMap);
      const newActive = new Set(state.activeCaptures);

      // Remove all entries for this messageId
      for (const key of newMap.keys()) {
        if (key.startsWith(`${messageId}:`)) {
          newMap.delete(key);
          newActive.delete(key);
        }
      }

      return { debugInfoMap: newMap, activeCaptures: newActive };
    });
  },

  clearAllDebugInfo: () => {
    set({ debugInfoMap: new Map(), activeCaptures: new Set() });
  },

  setConfig: (config) => {
    set((state) => ({
      config: { ...state.config, ...config },
    }));
  },
}));

// === Selectors ===

/** Get debug info for a specific message/model */
export const useDebugInfo = (
  messageId: string | undefined,
  model: string | undefined
): MessageDebugInfo | undefined => {
  return useDebugStore(
    useShallow((state) => {
      if (!messageId || !model) return undefined;
      return state.debugInfoMap.get(makeKey(messageId, model));
    })
  );
};

/** Check if there's any debug info for a message (any model) */
export const useHasDebugInfo = (messageId: string | undefined): boolean => {
  return useDebugStore(
    useShallow((state) => {
      if (!messageId) return false;
      for (const key of state.debugInfoMap.keys()) {
        if (key.startsWith(`${messageId}:`)) return true;
      }
      return false;
    })
  );
};

/** Get all debug info entries for a message (all models) */
export const useAllDebugInfoForMessage = (messageId: string | undefined): MessageDebugInfo[] => {
  return useDebugStore(
    useShallow((state) => {
      if (!messageId) return [];
      const results: MessageDebugInfo[] = [];
      for (const [key, info] of state.debugInfoMap.entries()) {
        if (key.startsWith(`${messageId}:`)) {
          results.push(info);
        }
      }
      return results;
    })
  );
};

/** Get debug capture configuration */
export const useDebugConfig = (): DebugCaptureConfig => {
  return useDebugStore(useShallow((state) => state.config));
};

/** Check if capture is currently active for a message/model */
export const useIsDebugCaptureActive = (messageId: string, model: string): boolean => {
  return useDebugStore((state) => state.activeCaptures.has(makeKey(messageId, model)));
};
