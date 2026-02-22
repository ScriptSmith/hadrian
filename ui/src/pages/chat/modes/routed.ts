import type {
  ModeContext,
  ModeResult,
  MessageUsage,
  ResponsesUsage,
  MessageModeMetadata,
} from "./types";
import { getContextInstances, findSpecialInstanceId } from "./types";
import type { ActiveModeState } from "@/stores/streamingStore";
import type { ModelInstance } from "@/components/chat-types";
import { DEFAULT_ROUTING_PROMPT } from "./prompts";
import { extractUserMessageText } from "./utils";
import { defineModeSpec, runMode } from "./runner";

/**
 * Routed mode state - matches the ActiveModeState variant for "routed"
 */
export type RoutedState = Extract<ActiveModeState, { mode: "routed" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface RoutedExecutionState extends RoutedState {
  _results?: Array<ModeResult | null>;
  _routerUsage?: MessageUsage;
}

/**
 * Routed mode specification.
 *
 * Flow:
 * 1. Router model analyzes the prompt and selects the best target model
 * 2. Selected model responds to the original prompt
 * 3. Returns the response with routing metadata
 */
export const routedSpec = defineModeSpec<RoutedState>({
  name: "routed",
  minModels: 1, // We handle single model case specially

  validate(_ctx) {
    // Always valid - single model case is handled in execute
    return true;
  },

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    const { modeConfig } = ctx;
    // Find router by instance ID, model ID, or fall back to first
    const routerInstanceId = findSpecialInstanceId(
      instances,
      modeConfig?.routerInstanceId,
      modeConfig?.routerModel
    );
    const routerInstance = instances.find((inst) => inst.id === routerInstanceId);

    return {
      mode: "routed",
      phase: "routing",
      routerModel: routerInstance?.modelId || instances[0]?.modelId,
      routerInstanceId: routerInstance?.id || instances[0]?.id,
      selectedModel: null,
      selectedInstanceId: null,
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, token, apiContent } = ctx;

    // Get instances and find router by instance ID
    const instances = runner.getInstances();
    const { routerModel, routerInstanceId } = runner.state;
    const routerInstance = instances.find((inst) => inst.id === routerInstanceId);

    // Target instances are all instances except the router instance
    const targetInstances = instances.filter((inst) => inst.id !== routerInstanceId);

    // If only the router instance(s) available, use the first router instance directly
    if (targetInstances.length === 0) {
      // Use the router instance (or create a minimal one if not found)
      const instanceToUse: ModelInstance = routerInstance || {
        id: routerInstanceId,
        modelId: routerModel,
      };

      const inputItems = runner.buildConversationInput(instanceToUse.modelId, apiContent!);
      const result = await runner.streamInstance({
        instance: instanceToUse,
        inputItems,
      });

      // Create final state with single instance result
      const finalState: RoutedExecutionState = {
        mode: "routed",
        phase: "selected",
        routerModel,
        routerInstanceId,
        selectedModel: instanceToUse.modelId,
        selectedInstanceId: instanceToUse.id,
        _results: instances.map((inst) => (inst.id === instanceToUse.id ? result : null)),
      };

      return finalState;
    }

    // Build list of target options for the router prompt
    // Use instance labels if available, otherwise model IDs
    const targetNames = targetInstances.map((inst) => inst.label || inst.modelId);

    // Build the routing prompt
    const routingPrompt =
      modeConfig?.routingPrompt ||
      DEFAULT_ROUTING_PROMPT.replace("{models}", targetNames.join("\n"));

    // Get the user message as text for routing analysis
    const userMessageText = extractUserMessageText(apiContent!);

    // Ask router to select the best model (non-streaming for speed)
    const routerController = new AbortController();
    ctx.abortControllersRef.current = [routerController];

    let selectedInstance: ModelInstance;
    let routingReasoning: string | undefined;
    let routerUsage: MessageUsage | undefined;
    let isFallback = false;

    try {
      const routerResponse = await fetch("/api/v1/responses", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          ...(token && { Authorization: `Bearer ${token}` }),
        },
        body: JSON.stringify({
          model: routerInstance?.modelId || routerModel,
          input: [
            { role: "system", content: routingPrompt },
            { role: "user", content: userMessageText },
          ],
          stream: false,
          max_output_tokens: 100, // Keep it short
          temperature: 0, // Deterministic routing
        }),
        signal: routerController.signal,
      });

      if (!routerResponse.ok) {
        throw new Error(`Router request failed: ${routerResponse.statusText}`);
      }

      const routerResult = (await routerResponse.json()) as {
        output_text?: string;
        output?: Array<{
          type?: string;
          role?: string;
          content?: Array<{ type?: string; text?: string }>;
        }>;
        usage?: ResponsesUsage;
      };

      // Extract the model selection and reasoning from the response
      let routerOutput = routerResult.output_text || "";
      let reasoningText = "";

      if (routerResult.output) {
        for (const outputItem of routerResult.output) {
          // Extract reasoning from reasoning blocks
          if (outputItem.type === "reasoning") {
            const reasoningContent = outputItem.content?.find((c) => c.type === "reasoning_text");
            if (reasoningContent?.text) {
              reasoningText = reasoningContent.text;
            }
            continue;
          }

          // Look for output_text in message blocks
          if (!routerOutput) {
            const textContent = outputItem.content?.find((c) => c.type === "output_text");
            if (textContent?.text) {
              routerOutput = textContent.text;
            }
          }
        }
      }

      // Capture router usage for cost tracking
      if (routerResult.usage) {
        routerUsage = {
          inputTokens: routerResult.usage.input_tokens,
          outputTokens: routerResult.usage.output_tokens,
          totalTokens: routerResult.usage.total_tokens,
          cost: routerResult.usage.cost,
          cachedTokens: routerResult.usage.input_tokens_details?.cached_tokens,
          reasoningTokens: routerResult.usage.output_tokens_details?.reasoning_tokens,
        };
      }

      // Parse the router's response - match against instance labels or model IDs
      const cleanedOutput = routerOutput.trim().toLowerCase();
      const matchedInstance = targetInstances.find((inst) => {
        const modelId = inst.modelId.toLowerCase();
        // Check model ID first (always present)
        if (cleanedOutput.includes(modelId) || cleanedOutput === modelId) {
          return true;
        }
        // Check label only if it exists and is non-empty
        if (inst.label) {
          const label = inst.label.toLowerCase();
          if (cleanedOutput.includes(label) || cleanedOutput === label) {
            return true;
          }
        }
        return false;
      });

      if (matchedInstance) {
        selectedInstance = matchedInstance;
        isFallback = false;
      } else {
        // Router returned invalid model, use fallback
        console.warn(`Router returned unrecognized model "${routerOutput.trim()}", using fallback`);
        selectedInstance = targetInstances[0];
        isFallback = true;
      }
      // Use reasoning text if available, otherwise fall back to the raw output
      routingReasoning = reasoningText || routerOutput.trim();
    } catch (error) {
      // On error, fallback to first available instance
      console.warn("Routing failed, using fallback:", error);
      selectedInstance = targetInstances[0];
      routingReasoning = "Routing failed, using default model";
      isFallback = true;
    }

    // Update routing state to show the decision
    runner.setState({
      mode: "routed",
      phase: "selected",
      routerModel,
      routerInstanceId,
      selectedModel: selectedInstance.modelId,
      selectedInstanceId: selectedInstance.id,
      reasoning: routingReasoning,
      isFallback,
    });

    // Stream from the selected instance using instance-aware streaming
    const inputItems = runner.buildConversationInput(selectedInstance.modelId, apiContent!);
    const result = await runner.streamInstance({
      instance: selectedInstance,
      inputItems,
    });

    // Build mode metadata for the response
    const modeMetadata: MessageModeMetadata = {
      mode: "routed",
      routerModel,
      routingReasoning,
      routerUsage,
    };

    // Create final state with results mapped to instance positions
    const finalState: RoutedExecutionState = {
      mode: "routed",
      phase: "selected",
      routerModel,
      routerInstanceId,
      selectedModel: selectedInstance.modelId,
      selectedInstanceId: selectedInstance.id,
      reasoning: routingReasoning,
      isFallback,
      _results: instances.map((inst) =>
        inst.id === selectedInstance.id && result ? { ...result, modeMetadata } : null
      ),
      _routerUsage: routerUsage,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as RoutedExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "routed" mode - a router model selects which model responds.
 *
 * Flow:
 * 1. Router model analyzes the prompt and selects the best target model
 * 2. Selected model responds to the original prompt
 * 3. Returns the response with routing metadata
 */
export async function sendRoutedMode(
  apiContent: string | unknown[],
  ctx: ModeContext
): Promise<Array<ModeResult | null>> {
  // Routed mode doesn't fallback - it handles all cases internally
  const noFallback = async () => [] as Array<ModeResult | null>;

  return runMode(routedSpec, apiContent, ctx, noFallback);
}
