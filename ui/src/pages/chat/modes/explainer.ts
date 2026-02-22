import type { ModeContext, ModeResult } from "./types";
import { getContextInstances } from "./types";
import type { ActiveModeState, ExplanationLevel } from "@/stores/streamingStore";
import type { ModelInstance } from "@/components/chat-types";
import {
  DEFAULT_EXPLAINER_INITIAL_PROMPT,
  DEFAULT_EXPLAINER_SIMPLIFY_PROMPT,
  DEFAULT_AUDIENCE_GUIDELINES,
} from "./prompts";
import { aggregateUsage, extractUserMessageText, messagesToInputItems } from "./utils";
import { defineModeSpec, runMode } from "./runner";

/**
 * Explainer mode state - matches the ActiveModeState variant for "explainer"
 */
export type ExplainerState = Extract<ActiveModeState, { mode: "explainer" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface ExplainerExecutionState extends ExplainerState {
  _results?: Array<ModeResult | null>;
}

/** Default audience levels for explainer mode */
export const DEFAULT_AUDIENCE_LEVELS = ["expert", "intermediate", "beginner"];

/**
 * Get guidelines for an audience level
 */
function getGuidelinesForLevel(level: string): string {
  const lowerLevel = level.toLowerCase();
  return (
    DEFAULT_AUDIENCE_GUIDELINES[lowerLevel] ||
    `- Explain the topic appropriately for a ${level} audience
- Adjust complexity and vocabulary accordingly
- Use relevant examples`
  );
}

/**
 * Capitalize an audience level for display
 */
function capitalizeLevel(level: string): string {
  return level.charAt(0).toUpperCase() + level.slice(1);
}

/**
 * Explainer mode specification.
 *
 * Flow:
 * 1. First instance explains the topic at the first audience level (e.g., expert)
 * 2. Subsequent instances simplify/adapt for each remaining audience level
 * 3. All explanations are displayed with their audience level
 *
 * The mode can work with a single instance (generating all levels) or multiple instances
 * (each instance handles one level). Cycles through instances if more levels than instances.
 *
 * Instance support:
 * - Uses instance-specific parameters for each level's generation
 * - Cycles through instances based on level index
 * - Includes instance labels in output metadata
 */
export const explainerSpec = defineModeSpec<ExplainerState>({
  name: "explainer",
  minModels: 1, // Only need one model/instance

  validate(ctx) {
    // Need at least one audience level
    const audienceLevels = ctx.modeConfig?.audienceLevels || DEFAULT_AUDIENCE_LEVELS;
    return audienceLevels.length > 0;
  },

  initialize(ctx) {
    const audienceLevels = ctx.modeConfig?.audienceLevels || DEFAULT_AUDIENCE_LEVELS;
    const instances = getContextInstances(ctx);
    const firstInstance = instances[0];

    return {
      mode: "explainer",
      phase: "initial",
      audienceLevels,
      currentLevelIndex: 0,
      explanations: [],
      currentModel: firstInstance.modelId,
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

    const { audienceLevels } = runner.state;

    // Get instances for cycling through
    const instances = runner.getInstances();

    // Get the user message as text for prompts
    const userMessageText = extractUserMessageText(apiContent!);

    // Track all explanations (extended to include instance info)
    interface ExtendedExplanationLevel extends ExplanationLevel {
      instance: ModelInstance;
    }
    const explanations: ExtendedExplanationLevel[] = [];
    let previousExplanation = "";

    // Generate explanations for each audience level sequentially
    for (let levelIndex = 0; levelIndex < audienceLevels.length; levelIndex++) {
      const level = audienceLevels[levelIndex];
      const levelGuidelines = getGuidelinesForLevel(level);

      // Determine which instance to use for this level
      // Cycle through available instances if there are fewer instances than levels
      const instanceIndex = levelIndex % instances.length;
      const instance = instances[instanceIndex];
      const model = instance.modelId;

      // Update state (only for subsequent levels; first level uses initial state from initialize)
      if (levelIndex > 0) {
        runner.setState({
          mode: "explainer",
          phase: "simplifying",
          audienceLevels,
          currentLevelIndex: levelIndex,
          explanations,
          currentModel: model,
        });
      }

      // Initialize streaming with instance ID and model mapping
      const modelMap = new Map<string, string>();
      modelMap.set(instance.id, model);
      streamingStore.initStreaming([instance.id], modelMap);

      // Create abort controller for this level
      const controller = new AbortController();
      abortControllersRef.current = [controller];

      // Build the prompt based on whether this is the first level or a subsequent one
      let prompt: string;
      if (levelIndex === 0) {
        // Initial explanation
        prompt = DEFAULT_EXPLAINER_INITIAL_PROMPT.replace("{level}", level)
          .replace("{question}", userMessageText)
          .replace("{level_guidelines}", levelGuidelines);
      } else {
        // Simplification/adaptation of previous explanation
        prompt = DEFAULT_EXPLAINER_SIMPLIFY_PROMPT.replace(/{level}/g, level)
          .replace("{question}", userMessageText)
          .replace("{previous_explanation}", previousExplanation)
          .replace("{level_guidelines}", levelGuidelines);
      }

      try {
        const filteredMessages = filterMessagesForModel(messages, model);
        const inputItems = [
          ...messagesToInputItems(filteredMessages),
          { role: "system", content: prompt },
          { role: "user", content: userMessageText },
        ];

        // Merge settings with instance parameters
        const instanceSettings = {
          ...settings,
          ...instance.parameters,
        };

        // Stream with instance ID and instance parameters
        const result = await streamResponse(
          model,
          inputItems,
          controller,
          instanceSettings,
          instance.id // Use instance ID as stream ID
        );

        if (result) {
          const explanation: ExtendedExplanationLevel = {
            level,
            model,
            content: result.content,
            usage: result.usage,
            instance,
          };
          explanations.push(explanation);
          previousExplanation = result.content;
          streamingStore.updateModeState((current) => {
            if (current.mode !== "explainer") return current;
            return {
              ...current,
              explanations: [
                ...current.explanations,
                { level, model, content: result.content, usage: result.usage },
              ],
            };
          });
        }
      } catch {
        // Continue to next level on error
      }
    }

    // Calculate aggregate usage
    const totalUsage = aggregateUsage(explanations);

    // Return all explanations as separate results (one per audience level)
    // Each explanation will appear as its own response card in the UI (like scattershot)
    const results: Array<ModeResult | null> = explanations.map((explanation, index) => ({
      content: explanation.content,
      usage: explanation.usage,
      // Store level info for display - levelLabel is used by useChat to determine the display name
      levelLabel: capitalizeLevel(explanation.level),
      modeMetadata: {
        mode: "explainer" as const,
        isExplanation: true,
        explainerLevel: explanation.level,
        explainerModel: explanation.model,
        explainerInstanceLabel: explanation.instance.label || explanation.model,
        // Include all explanations only on the first result for aggregate display
        ...(index === 0
          ? {
              explainerLevels: audienceLevels,
              explanations: explanations.map((e) => ({
                level: e.level,
                model: e.model,
                instanceLabel: e.instance.label,
                content: e.content,
                usage: e.usage,
              })),
              aggregateUsage: totalUsage,
            }
          : {}),
      },
    }));

    // Create final state
    const finalState: ExplainerExecutionState = {
      mode: "explainer",
      phase: "done",
      audienceLevels,
      currentLevelIndex: audienceLevels.length - 1,
      explanations,
      currentModel: undefined,
      _results: results,
    };

    return finalState;
  },

  finalize(state) {
    const execState = state as ExplainerExecutionState;
    return execState._results || [];
  },
});

/**
 * Send message in "explainer" mode - progressive explanation/simplification for different audiences.
 *
 * Flow:
 * 1. First model explains the topic at the first audience level (e.g., expert)
 * 2. Subsequent models simplify/adapt for each remaining audience level
 * 3. All explanations are displayed with their audience level
 *
 * The mode can work with a single model (generating all levels) or multiple models
 * (each model handles one level).
 */
export async function sendExplainerMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(explainerSpec, apiContent, ctx, sendMultipleMode);
}
