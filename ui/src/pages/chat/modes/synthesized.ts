import type { ModeContext, ModeResult, MessageUsage } from "./types";
import { getContextInstances, findSpecialInstanceId } from "./types";
import type { ActiveModeState } from "@/stores/streamingStore";
import { DEFAULT_SYNTHESIS_PROMPT } from "./prompts";
import { extractUserMessageText } from "./utils";
import { defineModeSpec, runMode, type InstanceGatherResult } from "./runner";

/**
 * Synthesized mode state - matches the ActiveModeState variant for "synthesized"
 */
export type SynthesizedState = Extract<ActiveModeState, { mode: "synthesized" }>;

/**
 * Synthesized mode specification.
 *
 * Flow:
 * 1. All non-synthesizer models respond in parallel (gathering phase)
 * 2. Synthesizer model combines all responses into a unified answer (synthesizing phase)
 * 3. Returns only the synthesized response with source responses in metadata
 */
export const synthesizedSpec = defineModeSpec<SynthesizedState>({
  name: "synthesized",
  minModels: 1, // We check for responding instances separately

  validate(ctx) {
    const instances = getContextInstances(ctx);
    const { modeConfig } = ctx;
    // Find synthesizer instance by instance ID, model ID, or fall back to first
    const synthesizerInstanceId = findSpecialInstanceId(
      instances,
      modeConfig?.synthesizerInstanceId,
      modeConfig?.synthesizerModel
    );
    const respondingInstances = instances.filter((inst) => inst.id !== synthesizerInstanceId);
    // Need at least one responding instance (besides the synthesizer)
    return respondingInstances.length > 0;
  },

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    const { modeConfig } = ctx;
    // Find synthesizer instance by instance ID, model ID, or fall back to first
    const synthesizerInstanceId = findSpecialInstanceId(
      instances,
      modeConfig?.synthesizerInstanceId,
      modeConfig?.synthesizerModel
    );
    const synthesizerInstance = instances.find((inst) => inst.id === synthesizerInstanceId);
    const respondingInstances = instances.filter((inst) => inst.id !== synthesizerInstanceId);

    return {
      mode: "synthesized",
      phase: "gathering",
      synthesizerModel: synthesizerInstance?.modelId || instances[0]?.modelId,
      synthesizerInstanceId: synthesizerInstance?.id || instances[0]?.id,
      completedModels: [],
      totalModels: respondingInstances.length,
      sourceResponses: [],
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, apiContent } = ctx;
    const { state } = runner;

    // Get instances and find synthesizer by instance ID
    const instances = runner.getInstances();
    const synthesizerInstanceId = state.synthesizerInstanceId;
    const synthesizerInstance = instances.find((inst) => inst.id === synthesizerInstanceId);
    const respondingInstances = instances.filter((inst) => inst.id !== synthesizerInstanceId);

    // Collect source responses during gathering
    const sourceResponses: Array<{ model: string; content: string; usage?: MessageUsage }> = [];

    // Gather responses from all non-synthesizer instances in parallel
    const gatherResult: InstanceGatherResult = await runner.gatherInstances({
      instances: respondingInstances,
      buildInputItems: (instance) => runner.buildConversationInput(instance.modelId, apiContent!),
      onInstanceComplete: (instance, result) => {
        if (result) {
          // Use instance label if available, otherwise model ID
          const displayName = instance.label || instance.modelId;
          const response = { model: displayName, content: result.content, usage: result.usage };
          sourceResponses.push(response);

          // Update state with completed model (use model ID for compatibility)
          runner.updateState((current) => ({
            ...current,
            completedModels: [...current.completedModels, instance.modelId],
            sourceResponses: [...current.sourceResponses, response],
          }));
        }
      },
    });

    // If no successful responses, stay in gathering phase with empty responses
    if (gatherResult.successfulResults.length === 0) {
      runner.updateState((current) => ({
        ...current,
        phase: "done",
      }));
      return runner.state;
    }

    // Update state to synthesizing phase
    runner.setState({
      mode: "synthesized",
      phase: "synthesizing",
      synthesizerModel: synthesizerInstance?.modelId || state.synthesizerModel,
      synthesizerInstanceId,
      completedModels: sourceResponses.map((r) => r.model),
      totalModels: respondingInstances.length,
      sourceResponses,
    });

    // Build the synthesis prompt - use display names (labels) for better context
    const responsesText = sourceResponses
      .map((r) => `[${r.model}]:\n${r.content}`)
      .join("\n\n---\n\n");

    const synthesisPrompt =
      modeConfig?.synthesisPrompt || DEFAULT_SYNTHESIS_PROMPT.replace("{responses}", responsesText);

    // Get the user message as text for synthesis context
    const userMessageText = extractUserMessageText(apiContent!);

    // Stream the synthesized response - synthesizer instance should always exist
    if (!synthesizerInstance) {
      throw new Error("Synthesizer instance not found");
    }

    const synthResult = await runner.streamInstance({
      instance: synthesizerInstance,
      inputItems: [
        { role: "system", content: synthesisPrompt },
        { role: "user", content: userMessageText },
      ],
    });

    // Mark synthesis as done and store the synthesis result
    const finalState: SynthesizedState = {
      mode: "synthesized",
      phase: "done",
      synthesizerModel: synthesizerInstance.modelId,
      synthesizerInstanceId,
      completedModels: sourceResponses.map((r) => r.model),
      totalModels: respondingInstances.length,
      sourceResponses,
    };

    runner.setState(finalState);

    // Store synthesis result for finalize
    (finalState as SynthesizedState & { _synthResult?: typeof synthResult })._synthResult =
      synthResult;

    return finalState;
  },

  finalize(state, ctx) {
    const instances = getContextInstances(ctx);
    const { synthesizerModel, synthesizerInstanceId, sourceResponses } = state;

    // Get synthesis result from state (stored during execute)
    const synthResult = (state as SynthesizedState & { _synthResult?: ModeResult | null })
      ._synthResult;

    // Return only the synthesized result (individual responses are in metadata)
    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);

    if (synthResult) {
      // Find the synthesizer instance by instance ID
      const synthesizerIndex = instances.findIndex((inst) => inst.id === synthesizerInstanceId);
      if (synthesizerIndex !== -1) {
        results[synthesizerIndex] = {
          ...synthResult,
          modeMetadata: {
            mode: "synthesized",
            isSynthesized: true,
            synthesizerModel,
            sourceResponses,
            synthesizerUsage: synthResult.usage,
          },
        };
      }
    }

    return results;
  },
});

/**
 * Send message in "synthesized" mode - all models respond in parallel,
 * then a synthesizer model combines the results.
 *
 * Flow:
 * 1. All non-synthesizer models respond in parallel (gathering phase)
 * 2. Synthesizer model combines all responses into a unified answer (synthesizing phase)
 * 3. Returns only the synthesized response with source responses in metadata
 */
export async function sendSynthesizedMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(synthesizedSpec, apiContent, ctx, sendMultipleMode);
}
