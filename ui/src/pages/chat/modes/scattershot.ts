import type { ModeContext, ModeResult } from "./types";
import { getContextInstances } from "./types";
import type { ActiveModeState, ScattershotVariation } from "@/stores/streamingStore";
import type { ModelParameters } from "@/components/chat-types";
import { aggregateUsage, extractUserMessageText, messagesToInputItems } from "./utils";
import { defineModeSpec, runMode } from "./runner";

/**
 * Scattershot mode state - matches the ActiveModeState variant for "scattershot"
 */
export type ScattershotState = Extract<ActiveModeState, { mode: "scattershot" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface ScattershotExecutionState extends ScattershotState {
  _results?: Array<ModeResult | null>;
}

/** Default parameter variations for scattershot mode */
export const DEFAULT_SCATTERSHOT_VARIATIONS: ModelParameters[] = [
  { temperature: 0.0 }, // Deterministic
  { temperature: 0.5 }, // Balanced
  { temperature: 1.0 }, // Creative
  { temperature: 1.5, topP: 0.9 }, // Very creative
];

/**
 * Generate variation labels based on parameters
 */
function getVariationLabel(params: ModelParameters, index: number): string {
  const parts: string[] = [];

  if (params.temperature !== undefined) {
    parts.push(`temp=${params.temperature}`);
  }
  if (params.topP !== undefined) {
    parts.push(`top_p=${params.topP}`);
  }
  if (params.topK !== undefined) {
    parts.push(`top_k=${params.topK}`);
  }
  if (params.frequencyPenalty !== undefined) {
    parts.push(`freq=${params.frequencyPenalty}`);
  }
  if (params.presencePenalty !== undefined) {
    parts.push(`pres=${params.presencePenalty}`);
  }
  if (params.maxTokens !== undefined) {
    parts.push(`max=${params.maxTokens}`);
  }

  if (parts.length === 0) {
    return `Variation ${index + 1}`;
  }

  return parts.join(", ");
}

/**
 * Scattershot mode specification.
 *
 * Flow:
 * 1. Take the first selected instance
 * 2. Run it N times in parallel with different parameter variations
 * 3. Display all results side-by-side for comparison
 *
 * This mode is useful for:
 * - Comparing creative vs. deterministic outputs
 * - Finding the best parameter settings for a task
 * - Generating multiple variations of content
 *
 * Instance support:
 * - Uses the first instance's base parameters (temperature, etc.)
 * - Variation parameters override the instance's base parameters
 * - Instance label is included in output metadata
 */
export const scattershotSpec = defineModeSpec<ScattershotState>({
  name: "scattershot",
  minModels: 1, // Only need one model/instance

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    const targetInstance = instances[0];
    const targetModel = targetInstance.modelId;

    // Get parameter variations (use custom or defaults)
    const variations = ctx.modeConfig?.parameterVariations?.length
      ? ctx.modeConfig.parameterVariations
      : DEFAULT_SCATTERSHOT_VARIATIONS;

    // Create variation identifiers with labels
    const scattershotVariations: ScattershotVariation[] = variations.map((params, index) => ({
      id: `${targetInstance.id}__variation_${index}`,
      index,
      params,
      label: getVariationLabel(params, index),
      status: "pending",
    }));

    return {
      mode: "scattershot",
      phase: "generating",
      targetModel,
      variations: scattershotVariations,
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

    const { targetModel, variations } = runner.state;

    // Get the target instance for base parameters
    const instances = runner.getInstances();
    const targetInstance = instances[0];

    // Initialize streaming for all variations (using variation IDs as model keys)
    // Map variation IDs to the target model for proper streaming display
    const modelMap = new Map<string, string>();
    for (const v of variations) {
      modelMap.set(v.id, targetModel);
    }
    streamingStore.initStreaming(
      variations.map((v) => v.id),
      modelMap
    );

    // Create abort controllers for each variation
    const controllers = variations.map(() => new AbortController());
    abortControllersRef.current = controllers;

    // Get the user message for prompts
    const userMessageText = extractUserMessageText(apiContent!);

    // Run all variations in parallel
    const variationPromises = variations.map(async (variation, index) => {
      // Mark as generating
      streamingStore.updateModeState((current) => {
        if (current.mode !== "scattershot") return current;
        return {
          ...current,
          variations: current.variations.map((v) =>
            v.id === variation.id ? { ...v, status: "generating" as const } : v
          ),
        };
      });

      // Build input items
      const filteredMessages = filterMessagesForModel(messages, targetModel);
      const inputItems = [
        ...messagesToInputItems(filteredMessages),
        { role: "user", content: userMessageText },
      ];

      // Merge: base settings -> instance parameters -> variation params
      // Variation params override instance params which override base settings
      const variationSettings = {
        ...settings,
        ...targetInstance.parameters,
        ...variation.params,
      };

      try {
        const result = await streamResponse(
          targetModel, // Use actual model for API request
          inputItems,
          controllers[index],
          variationSettings,
          variation.id // Use variation ID for streaming store
        );

        if (result) {
          streamingStore.updateModeState((current) => {
            if (current.mode !== "scattershot") return current;
            return {
              ...current,
              variations: current.variations.map((v) =>
                v.id === variation.id
                  ? {
                      ...v,
                      status: "complete" as const,
                      content: result.content,
                      usage: result.usage,
                    }
                  : v
              ),
            };
          });
          return {
            content: result.content,
            usage: result.usage,
            variationId: variation.id,
            params: variation.params,
            label: variation.label,
          };
        } else {
          streamingStore.updateModeState((current) => {
            if (current.mode !== "scattershot") return current;
            return {
              ...current,
              variations: current.variations.map((v) =>
                v.id === variation.id ? { ...v, status: "failed" as const } : v
              ),
            };
          });
          return null;
        }
      } catch {
        streamingStore.updateModeState((current) => {
          if (current.mode !== "scattershot") return current;
          return {
            ...current,
            variations: current.variations.map((v) =>
              v.id === variation.id ? { ...v, status: "failed" as const } : v
            ),
          };
        });
        return null;
      }
    });

    const variationResults = await Promise.all(variationPromises);

    // Update final scattershot state
    const completedVariations = variations.map((v, index) => {
      const result = variationResults[index];
      if (result) {
        return {
          ...v,
          status: "complete" as const,
          content: result.content,
          usage: result.usage,
        };
      }
      return {
        ...v,
        status: "failed" as const,
      };
    });

    // Calculate aggregate usage
    const totalUsage = aggregateUsage(completedVariations);

    // Get instance label for metadata (use label if available, otherwise model ID)
    const instanceLabel = targetInstance.label || targetModel;

    // Return all variations as separate results (one per variation, not per model)
    // Each variation will appear as its own response card in the UI
    const results: Array<ModeResult | null> = variationResults.map((result, index) => {
      if (result) {
        return {
          content: result.content,
          usage: result.usage,
          // Store variation info in modeMetadata for display
          modeMetadata: {
            mode: "scattershot" as const,
            isScattershot: true,
            scattershotModel: targetModel,
            scattershotInstanceLabel: instanceLabel,
            // Include this variation's details
            scattershotVariationLabel: result.label,
            scattershotVariationParams: result.params,
            // Include all variations only on the first result for aggregate display
            ...(index === 0
              ? {
                  scattershotVariations: completedVariations.map((v) => ({
                    id: v.id,
                    index: v.index,
                    params: v.params,
                    label: v.label,
                    content: v.content,
                    usage: v.usage,
                  })),
                  aggregateUsage: totalUsage,
                }
              : {}),
          },
          // Use variation ID as the "model" for display purposes
          variationId: result.variationId,
          variationLabel: result.label,
        };
      }
      return null;
    });

    // Create final state
    const finalState: ScattershotExecutionState = {
      mode: "scattershot",
      phase: "done",
      targetModel,
      variations: completedVariations,
      _results: results,
    };

    return finalState;
  },

  finalize(state) {
    const execState = state as ScattershotExecutionState;
    return execState._results || [];
  },
});

/**
 * Send message in "scattershot" mode - run the same model multiple times with different parameters.
 *
 * Flow:
 * 1. Take the first selected model
 * 2. Run it N times in parallel with different parameter variations
 * 3. Display all results side-by-side for comparison
 *
 * This mode is useful for:
 * - Comparing creative vs. deterministic outputs
 * - Finding the best parameter settings for a task
 * - Generating multiple variations of content
 */
export async function sendScattershotMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(scattershotSpec, apiContent, ctx, sendMultipleMode);
}
