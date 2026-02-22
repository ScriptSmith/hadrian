import type { ModeContext, ModeResult, RefinementRoundData, MessageUsage } from "./types";
import { getContextInstances } from "./types";
import type { ActiveModeState } from "@/stores/streamingStore";
import type { ModelInstance } from "@/components/chat-types";
import { DEFAULT_REFINEMENT_PROMPT } from "./prompts";
import { extractUserMessageText, messagesToInputItems } from "./utils";
import { defineModeSpec, runMode } from "./runner";

/**
 * Refined mode state - matches the ActiveModeState variant for "refined"
 */
export type RefinedState = Extract<ActiveModeState, { mode: "refined" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface RefinedExecutionState extends RefinedState {
  _results?: Array<ModeResult | null>;
}

/**
 * Refined mode specification.
 *
 * Flow:
 * 1. First model generates initial response to the prompt
 * 2. Each subsequent model refines/improves the previous response
 * 3. Process continues for `refinementRounds` iterations
 * 4. Returns only the final refined response with refinement history in metadata
 */
export const refinedSpec = defineModeSpec<RefinedState>({
  name: "refined",
  minModels: 2, // Need at least 2 models for refinement

  validate(ctx) {
    const instances = getContextInstances(ctx);
    const { modeConfig } = ctx;
    const totalRounds = Math.min(modeConfig?.refinementRounds ?? 2, instances.length);
    return totalRounds >= 2;
  },

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    const { modeConfig } = ctx;
    const totalRounds = Math.min(modeConfig?.refinementRounds ?? 2, instances.length);

    return {
      mode: "refined",
      phase: "initial",
      currentRound: 0,
      totalRounds,
      currentModel: instances[0].id, // Use instance ID for tracking
      rounds: [],
    };
  },

  async execute(ctx, runner) {
    const {
      messages,
      settings,
      modeConfig,
      streamingStore,
      abortControllersRef,
      streamResponse,
      filterMessagesForModel,
      apiContent,
    } = ctx;

    // Get instances from runner (with backwards compatibility)
    const instances = runner.getInstances();
    const totalRounds = runner.state.totalRounds;

    // Track all refinement rounds
    const refinementHistory: RefinementRoundData[] = [];

    // Get user message as text for context
    const userMessageText = extractUserMessageText(apiContent!);

    // Round 0: Initial response from first instance
    const initialInstance = instances[0];

    // Initialize streaming with instance ID and model mapping
    const modelMap = new Map<string, string>();
    modelMap.set(initialInstance.id, initialInstance.modelId);
    streamingStore.initStreaming([initialInstance.id], modelMap);

    const initialController = new AbortController();
    abortControllersRef.current = [initialController];

    const filteredMessages = filterMessagesForModel(messages, initialInstance.modelId);
    const initialInputItems = [
      ...messagesToInputItems(filteredMessages),
      { role: "user", content: apiContent! },
    ];

    const initialResult = await streamResponse(
      initialInstance.modelId, // Use model ID for API call
      initialInputItems,
      initialController,
      settings,
      initialInstance.id, // Use instance ID for streaming store
      undefined, // trackToolCalls
      undefined, // onSSEEvent
      initialInstance.parameters // Pass instance-specific parameters
    );

    if (!initialResult) {
      // Initial response failed
      const finalState: RefinedExecutionState = {
        ...runner.state,
        phase: "done",
        _results: new Array(instances.length).fill(null),
      };
      return finalState;
    }

    // Record initial round (use instance label or ID for display)
    const initialRound: RefinementRoundData = {
      model: initialInstance.label || initialInstance.id,
      content: initialResult.content,
      usage: initialResult.usage,
    };
    refinementHistory.push(initialRound);
    runner.updateState((current) => ({
      ...current,
      rounds: [...current.rounds, initialRound],
    }));

    // Current response to refine
    let currentContent = initialResult.content;
    let finalInstance: ModelInstance = initialInstance;
    let finalUsage: MessageUsage | undefined = initialResult.usage;

    // Subsequent refinement rounds
    for (let round = 1; round < totalRounds; round++) {
      // Check if aborted
      if (abortControllersRef.current[0]?.signal.aborted) {
        break;
      }

      // Select the next instance (cycle through available instances)
      const refiningInstance = instances[round % instances.length];
      finalInstance = refiningInstance;

      // Update refinement state
      runner.updateState((current) => ({
        ...current,
        phase: "refining" as const,
        currentRound: round,
        currentModel: refiningInstance.id,
      }));

      // Build refinement prompt
      const refinementPrompt =
        modeConfig?.refinementPrompt ||
        DEFAULT_REFINEMENT_PROMPT.replace("{previous_response}", currentContent);

      // Initialize streaming for refining instance
      const refineModelMap = new Map<string, string>();
      refineModelMap.set(refiningInstance.id, refiningInstance.modelId);
      streamingStore.initStreaming([refiningInstance.id], refineModelMap);

      const refineController = new AbortController();
      abortControllersRef.current = [refineController];

      // Stream the refined response (with instance parameters)
      const refineResult = await streamResponse(
        refiningInstance.modelId, // Use model ID for API call
        [
          { role: "system", content: refinementPrompt },
          { role: "user", content: userMessageText },
        ],
        refineController,
        settings,
        refiningInstance.id, // Use instance ID for streaming store
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        refiningInstance.parameters // Pass instance-specific parameters
      );

      if (refineResult) {
        // Record refinement round (use instance label or ID for display)
        const refineRound: RefinementRoundData = {
          model: refiningInstance.label || refiningInstance.id,
          content: refineResult.content,
          usage: refineResult.usage,
        };
        refinementHistory.push(refineRound);
        runner.updateState((current) => ({
          ...current,
          rounds: [...current.rounds, refineRound],
        }));

        // Update current content for next round
        currentContent = refineResult.content;
        finalUsage = refineResult.usage;
      } else {
        // Refinement failed, use previous content
        break;
      }
    }

    // Build final result - find the final instance index
    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
    const finalInstanceIndex = instances.findIndex((inst) => inst.id === finalInstance.id);

    if (finalInstanceIndex !== -1) {
      results[finalInstanceIndex] = {
        content: currentContent,
        usage: finalUsage,
        modeMetadata: {
          mode: "refined",
          isRefined: true,
          refinementRound: refinementHistory.length - 1,
          totalRounds: refinementHistory.length,
          refinementHistory,
        },
      };
    }

    // Create final state
    const finalState: RefinedExecutionState = {
      mode: "refined",
      phase: "done",
      currentRound: refinementHistory.length - 1,
      totalRounds,
      currentModel: finalInstance.id,
      rounds: refinementHistory,
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as RefinedExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "refined" mode - models take turns refining the response.
 *
 * Flow:
 * 1. First model generates initial response to the prompt
 * 2. Each subsequent model refines/improves the previous response
 * 3. Process continues for `refinementRounds` iterations
 * 4. Returns only the final refined response with refinement history in metadata
 */
export async function sendRefinedMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(refinedSpec, apiContent, ctx, sendMultipleMode);
}
