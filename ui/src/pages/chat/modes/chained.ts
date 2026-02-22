import type { ModeContext, ModeResult, MessageUsage } from "./types";
import { getContextInstances } from "./types";
import type { ActiveModeState } from "@/stores/streamingStore";
import type { ModelInstance } from "@/components/chat-types";
import { defineModeSpec, runMode } from "./runner";
import { messagesToInputItems } from "./utils";

/**
 * Chained mode state - matches the ActiveModeState variant for "chained"
 */
export type ChainedState = Extract<ActiveModeState, { mode: "chained" }>;

/**
 * Extended state for tracking chain execution (internal use in finalize)
 */
interface ChainedExecutionState extends ChainedState {
  _results?: Array<ModeResult | null>;
}

/**
 * Chained mode specification.
 *
 * Flow:
 * 1. First model responds to the original prompt
 * 2. Each subsequent model sees all previous responses and builds upon them
 * 3. Returns all responses with chain position metadata
 */
export const chainedSpec = defineModeSpec<ChainedState>({
  name: "chained",
  minModels: 1, // Works with any number of models

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    return {
      mode: "chained",
      position: [0, instances.length] as [number, number],
    };
  },

  async execute(ctx, runner) {
    const {
      messages,
      settings,
      streamingStore,
      abortControllersRef,
      streamResponse,
      filterMessagesForModel,
      apiContent,
    } = ctx;

    // Get instances from runner (with backwards compatibility)
    const instances = runner.getInstances();

    // Initialize streaming for all instances (using instance IDs)
    const instanceIds = instances.map((inst) => inst.id);
    const modelMap = new Map<string, string>();
    for (const inst of instances) {
      modelMap.set(inst.id, inst.modelId);
    }
    streamingStore.initStreaming(instanceIds, modelMap);

    // Track chain responses and results
    const chainResponses: Array<{
      instance: ModelInstance;
      content: string;
      usage?: MessageUsage;
    }> = [];
    const results: Array<ModeResult | null> = [];

    // Create initial abort controller
    let controller = new AbortController();
    abortControllersRef.current = [controller];

    for (let i = 0; i < instances.length; i++) {
      const instance = instances[i];

      // Check if aborted
      if (controller.signal.aborted) {
        results.push(null);
        continue;
      }

      // Update chain position
      runner.setState({
        mode: "chained",
        position: [i, instances.length] as [number, number],
      });

      // Build input items:
      // - Start with conversation history (filtered by historyMode)
      // - Add the original user message
      // - Add all previous chain responses
      const filteredMessages = filterMessagesForModel(messages, instance.modelId);
      const inputItems: Array<{ role: string; content: string | unknown[] }> = [
        ...messagesToInputItems(filteredMessages),
        { role: "user", content: apiContent! },
      ];

      // Add previous chain responses as context
      for (const prev of chainResponses) {
        // Use instance label if available, otherwise model ID
        const label = prev.instance.label || prev.instance.modelId;
        inputItems.push({
          role: "assistant",
          content: `[${label}]: ${prev.content}`,
        });
      }

      // If not the first instance, add instruction to build on previous responses
      if (i > 0) {
        inputItems.push({
          role: "user",
          content:
            "Please build upon, refine, or improve the previous response(s). You can add new insights, correct errors, or expand on ideas.",
        });
      }

      // Create a new controller for this instance
      controller = new AbortController();
      abortControllersRef.current = [controller];

      // Stream this instance's response (passing instance parameters)
      const result = await streamResponse(
        instance.modelId, // Use model ID for API call
        inputItems,
        controller,
        settings,
        instance.id, // Use instance ID for streaming store
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        instance.parameters, // Pass instance-specific parameters
        instance.label // Pass instance label for system prompt
      );

      if (result !== null) {
        // Add mode metadata to result
        results.push({
          ...result,
          modeMetadata: {
            mode: "chained",
            chainPosition: i,
            chainTotal: instances.length,
          },
        });
        chainResponses.push({ instance, content: result.content, usage: result.usage });
      } else {
        results.push(null);
      }
    }

    // Create final state with results attached for finalize
    // Use a fresh object to avoid mutating the runner's state
    const finalState: ChainedExecutionState = {
      mode: "chained",
      position: [instances.length - 1, instances.length] as [number, number],
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    // Retrieve results from execution state
    const execState = state as ChainedExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "chained" mode - models respond sequentially,
 * each seeing the previous model's response.
 *
 * Flow:
 * 1. First model responds to the original prompt
 * 2. Each subsequent model sees all previous responses and builds upon them
 * 3. Returns all responses with chain position metadata
 */
export async function sendChainedMode(
  apiContent: string | unknown[],
  ctx: ModeContext
): Promise<Array<ModeResult | null>> {
  // Chained mode doesn't have a fallback - it works with any number of models
  const noFallback = async () => [] as Array<ModeResult | null>;

  return runMode(chainedSpec, apiContent, ctx, noFallback);
}
