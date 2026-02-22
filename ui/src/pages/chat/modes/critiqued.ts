import type { ModeContext, ModeResult, CritiqueRoundData } from "./types";
import { getContextInstances, findSpecialInstanceId } from "./types";
import type { ActiveModeState, CritiqueData } from "@/stores/streamingStore";
import type { ModelInstance } from "@/components/chat-types";
import { DEFAULT_CRITIQUE_PROMPT, DEFAULT_REVISION_PROMPT } from "./prompts";
import { extractUserMessageText } from "./utils";
import { defineModeSpec, runMode, type InstanceGatherResult } from "./runner";

/**
 * Critiqued mode state - matches the ActiveModeState variant for "critiqued"
 */
export type CritiquedState = Extract<ActiveModeState, { mode: "critiqued" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface CritiquedExecutionState extends CritiquedState {
  _results?: Array<ModeResult | null>;
}

/**
 * Critiqued mode specification.
 *
 * Flow:
 * 1. Primary model generates initial response
 * 2. Critic models (all other selected models) provide critiques in parallel
 * 3. Primary model revises based on the critiques
 * 4. Returns only the final revised response with initial response and critiques in metadata
 */
export const critiquedSpec = defineModeSpec<CritiquedState>({
  name: "critiqued",
  minModels: 1, // We need custom validation since we need at least 1 critic

  validate(ctx) {
    const instances = getContextInstances(ctx);
    // Determine primary instance by instance ID, model ID, or fall back to first
    const primaryInstanceId = findSpecialInstanceId(
      instances,
      ctx.modeConfig?.primaryInstanceId,
      ctx.modeConfig?.primaryModel
    );
    const critiqueInstances = instances.filter((inst) => inst.id !== primaryInstanceId);
    // Need at least one critic instance
    return critiqueInstances.length > 0;
  },

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    // Find primary instance by instance ID, model ID, or fall back to first
    const primaryInstanceId = findSpecialInstanceId(
      instances,
      ctx.modeConfig?.primaryInstanceId,
      ctx.modeConfig?.primaryModel
    );
    const primaryInstance = instances.find((inst) => inst.id === primaryInstanceId);
    const critiqueInstances = instances.filter((inst) => inst.id !== primaryInstanceId);

    return {
      mode: "critiqued",
      phase: "initial",
      primaryModel: primaryInstance?.modelId || instances[0]?.modelId,
      primaryInstanceId: primaryInstance?.id || instances[0]?.id,
      critiqueModels: critiqueInstances.map((inst) => inst.modelId),
      critiques: [],
      completedCritiques: 0,
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, streamingStore, apiContent } = ctx;

    const { primaryModel, primaryInstanceId } = runner.state;

    // Get instances and find primary + critics by instance ID
    const instances = runner.getInstances();
    const primaryInstance = instances.find((inst) => inst.id === primaryInstanceId);
    const critiqueInstances = instances.filter((inst) => inst.id !== primaryInstanceId);

    // Get user message as text for prompts
    const userMessageText = extractUserMessageText(apiContent!);

    // Phase 1: Generate initial response from primary instance
    if (!primaryInstance) {
      // No primary instance found - return nulls
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      const finalState: CritiquedExecutionState = {
        ...runner.state,
        phase: "done",
        _results: results,
      };
      return finalState;
    }

    const initialResult = await runner.streamInstance({
      instance: primaryInstance,
      inputItems: runner.buildConversationInput(primaryInstance.modelId, apiContent!),
    });

    if (!initialResult) {
      // Initial response failed - return nulls
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      const finalState: CritiquedExecutionState = {
        ...runner.state,
        phase: "done",
        _results: results,
      };
      return finalState;
    }

    const initialResponse = initialResult.content;
    const initialUsage = initialResult.usage;

    // Update state with initial response, move to critiquing phase
    runner.setState({
      mode: "critiqued",
      phase: "critiquing",
      primaryModel,
      primaryInstanceId,
      critiqueModels: critiqueInstances.map((inst) => inst.modelId),
      initialResponse,
      initialUsage,
      critiques: [],
      completedCritiques: 0,
    });

    // Phase 2: Gather critiques from all critic instances in parallel
    // Build critique prompt
    const critiquePrompt =
      modeConfig?.critiquePrompt || DEFAULT_CRITIQUE_PROMPT.replace("{response}", initialResponse);

    // Collect critiques using instance-aware gathering
    const critiques: CritiqueData[] = [];

    const gatherResult: InstanceGatherResult = await runner.gatherInstances({
      instances: critiqueInstances,
      buildInputItems: () => [
        { role: "system", content: critiquePrompt },
        { role: "user", content: userMessageText },
      ],
      onInstanceComplete: (instance: ModelInstance, result) => {
        if (result) {
          // Use instance label if available, otherwise model ID
          const displayName = instance.label || instance.modelId;
          const critique: CritiqueData = {
            model: displayName,
            content: result.content,
            usage: result.usage,
          };
          critiques.push(critique);
          streamingStore.updateModeState((current) => {
            if (current.mode !== "critiqued") return current;
            return {
              ...current,
              critiques: [...current.critiques, critique],
              completedCritiques: current.completedCritiques + 1,
            };
          });
        }
      },
    });

    // If no critiques received, return initial response as final
    if (gatherResult.successfulResults.length === 0) {
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      const primaryIndex = instances.findIndex((inst) => inst.id === primaryInstanceId);
      if (primaryIndex !== -1) {
        results[primaryIndex] = {
          content: initialResponse,
          usage: initialUsage,
          modeMetadata: {
            mode: "critiqued",
            isCritiqued: true,
            primaryModel,
            initialResponse,
            initialUsage,
            critiques: [],
          },
        };
      }

      const finalState: CritiquedExecutionState = {
        mode: "critiqued",
        phase: "done",
        primaryModel,
        primaryInstanceId,
        critiqueModels: critiqueInstances.map((inst) => inst.modelId),
        initialResponse,
        initialUsage,
        critiques: [],
        completedCritiques: 0,
        _results: results,
      };
      return finalState;
    }

    // Phase 3: Primary instance revises based on critiques
    runner.setState({
      mode: "critiqued",
      phase: "revising",
      primaryModel,
      primaryInstanceId,
      critiqueModels: critiqueInstances.map((inst) => inst.modelId),
      initialResponse,
      initialUsage,
      critiques,
      completedCritiques: critiques.length,
    });

    // Build revision prompt with critiques (use display names for context)
    const critiquesText = critiques.map((c) => `[${c.model}]:\n${c.content}`).join("\n\n---\n\n");
    const revisionPrompt = DEFAULT_REVISION_PROMPT.replace(
      "{original_response}",
      initialResponse
    ).replace("{critiques}", critiquesText);

    const revisionResult = await runner.streamInstance({
      instance: primaryInstance,
      inputItems: [
        { role: "system", content: revisionPrompt },
        { role: "user", content: userMessageText },
      ],
    });

    // Build final results
    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
    const primaryIndex = instances.findIndex((inst) => inst.id === primaryInstanceId);

    if (primaryIndex !== -1) {
      const finalContent = revisionResult?.content || initialResponse;
      const finalUsage = revisionResult?.usage;

      // Convert CritiqueData to CritiqueRoundData for metadata
      const critiqueRoundData: CritiqueRoundData[] = critiques.map((c) => ({
        model: c.model,
        content: c.content,
        usage: c.usage,
      }));

      results[primaryIndex] = {
        content: finalContent,
        usage: finalUsage,
        modeMetadata: {
          mode: "critiqued",
          isCritiqued: true,
          primaryModel,
          initialResponse,
          initialUsage,
          critiques: critiqueRoundData,
        },
      };
    }

    // Create final state
    const finalState: CritiquedExecutionState = {
      mode: "critiqued",
      phase: "done",
      primaryModel,
      primaryInstanceId,
      critiqueModels: critiqueInstances.map((inst) => inst.modelId),
      initialResponse,
      initialUsage,
      critiques,
      completedCritiques: critiques.length,
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as CritiquedExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "critiqued" mode - one model responds, others critique, then revision.
 *
 * Flow:
 * 1. Primary model generates initial response
 * 2. Critic models (all other selected models) provide critiques in parallel
 * 3. Primary model revises based on the critiques
 * 4. Returns only the final revised response with initial response and critiques in metadata
 */
export async function sendCritiquedMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(critiquedSpec, apiContent, ctx, sendMultipleMode);
}
