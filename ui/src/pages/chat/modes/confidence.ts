import type { ModeContext, ModeResult } from "./types";
import { getContextInstances, findSpecialInstanceId } from "./types";
import type { ActiveModeState, ConfidenceResponse } from "@/stores/streamingStore";
import { DEFAULT_CONFIDENCE_RESPONSE_PROMPT, DEFAULT_CONFIDENCE_SYNTHESIS_PROMPT } from "./prompts";
import { extractUserMessageText } from "./utils";
import { defineModeSpec, runMode } from "./runner";

/**
 * Confidence mode state - matches the ActiveModeState variant for "confidence-weighted"
 */
export type ConfidenceState = Extract<ActiveModeState, { mode: "confidence-weighted" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface ConfidenceExecutionState extends ConfidenceState {
  _results?: Array<ModeResult | null>;
}

/**
 * Confidence-weighted mode specification.
 *
 * Flow:
 * 1. All non-synthesizer models respond with self-assessed confidence scores (responding phase)
 * 2. Synthesizer model combines all responses weighted by confidence (synthesizing phase)
 * 3. Returns only the synthesized response with source responses in metadata
 */
export const confidenceSpec = defineModeSpec<ConfidenceState>({
  name: "confidence-weighted",
  minModels: 1, // We need custom validation

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
      mode: "confidence-weighted",
      phase: "responding",
      responses: [],
      completedResponses: 0,
      totalModels: respondingInstances.length,
      synthesizerModel: synthesizerInstance?.modelId || instances[0]?.modelId,
      synthesizerInstanceId: synthesizerInstance?.id || instances[0]?.id,
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, streamingStore, apiContent } = ctx;

    const { synthesizerModel, synthesizerInstanceId, totalModels } = runner.state;

    // Get instances and find synthesizer by instance ID
    const instances = runner.getInstances();
    const synthesizerInstance = instances.find((inst) => inst.id === synthesizerInstanceId);
    const respondingInstances = instances.filter((inst) => inst.id !== synthesizerInstanceId);

    // Get the user message as text
    const userMessageText = extractUserMessageText(apiContent!);

    // Collect responses with confidence scores
    const responses: ConfidenceResponse[] = [];

    // Build the confidence prompt
    const confidencePrompt =
      modeConfig?.confidencePrompt ||
      DEFAULT_CONFIDENCE_RESPONSE_PROMPT.replace("{question}", userMessageText);

    // Gather responses from all responding instances in parallel
    const gatherResult = await runner.gatherInstances({
      instances: respondingInstances,
      buildInputItems: (instance) => {
        const conversationInput = runner.buildConversationInput(instance.modelId, apiContent!);
        // Insert system prompt before the user content
        return [
          ...conversationInput.slice(0, -1),
          { role: "system", content: confidencePrompt },
          conversationInput[conversationInput.length - 1],
        ];
      },
      onInstanceComplete: (instance, result) => {
        if (result) {
          // Parse confidence score from response
          const { content, confidence } = parseConfidenceResponse(result.content);

          // Use instance label for display
          const displayName = instance.label || instance.modelId;
          const response: ConfidenceResponse = {
            model: displayName,
            content,
            confidence,
            usage: result.usage,
          };
          responses.push(response);
          streamingStore.updateModeState((current) => {
            if (current.mode !== "confidence-weighted") return current;
            return {
              ...current,
              responses: [...current.responses, response],
              completedResponses: current.completedResponses + 1,
            };
          });
        }
      },
    });

    // If no successful responses, return empty results
    if (gatherResult.successfulResults.length === 0) {
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      const finalState: ConfidenceExecutionState = {
        mode: "confidence-weighted",
        phase: "done",
        responses: [],
        completedResponses: 0,
        totalModels,
        synthesizerModel,
        synthesizerInstanceId,
        _results: results,
      };
      return finalState;
    }

    // Update state to synthesizing phase
    runner.setState({
      mode: "confidence-weighted",
      phase: "synthesizing",
      responses,
      completedResponses: responses.length,
      totalModels,
      synthesizerModel,
      synthesizerInstanceId,
    });

    // Build the synthesis prompt with confidence scores
    // Sort by confidence (highest first) for the synthesizer to see
    const sortedResponses = [...responses].sort((a, b) => b.confidence - a.confidence);
    const responsesText = sortedResponses
      .map((r) => `[${r.model}] (Confidence: ${(r.confidence * 100).toFixed(0)}%):\n${r.content}`)
      .join("\n\n---\n\n");

    const synthesisPrompt =
      modeConfig?.synthesisPrompt ||
      DEFAULT_CONFIDENCE_SYNTHESIS_PROMPT.replace("{responses}", responsesText);

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

    // Return only the synthesized result (individual responses are in metadata)
    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);

    if (synthResult) {
      // Find the synthesizer instance by instance ID
      const synthesizerIndex = instances.findIndex((inst) => inst.id === synthesizerInstanceId);
      if (synthesizerIndex !== -1) {
        results[synthesizerIndex] = {
          ...synthResult,
          modeMetadata: {
            mode: "confidence-weighted",
            isConfidenceWeighted: true,
            synthesizerModel,
            confidenceResponses: responses.map((r) => ({
              model: r.model,
              content: r.content,
              confidence: r.confidence,
              usage: r.usage,
            })),
            synthesizerUsage: synthResult.usage,
          },
        };
      }
    }

    // Create final state
    const finalState: ConfidenceExecutionState = {
      mode: "confidence-weighted",
      phase: "done",
      responses,
      completedResponses: responses.length,
      totalModels,
      synthesizerModel,
      synthesizerInstanceId,
      synthesis: synthResult?.content,
      synthesisUsage: synthResult?.usage,
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as ConfidenceExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "confidence-weighted" mode - all models respond with confidence scores,
 * then a synthesizer combines the results weighted by confidence.
 *
 * Flow:
 * 1. All non-synthesizer models respond with self-assessed confidence scores (responding phase)
 * 2. Synthesizer model combines all responses weighted by confidence (synthesizing phase)
 * 3. Returns only the synthesized response with source responses in metadata
 */
export async function sendConfidenceWeightedMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(confidenceSpec, apiContent, ctx, sendMultipleMode);
}

/**
 * Parse confidence score from a response.
 * Looks for "CONFIDENCE: X.X" pattern at the end of the response.
 * Returns the content without the confidence line and the parsed score.
 */
function parseConfidenceResponse(fullContent: string): { content: string; confidence: number } {
  // Default confidence if not found (moderate)
  let confidence = 0.5;
  let content = fullContent;

  // Look for CONFIDENCE: pattern (case-insensitive)
  const confidenceMatch = fullContent.match(/\n?\s*CONFIDENCE:\s*([\d.]+)\s*$/i);
  if (confidenceMatch) {
    const parsedScore = parseFloat(confidenceMatch[1]);
    if (!isNaN(parsedScore) && parsedScore >= 0 && parsedScore <= 1) {
      confidence = parsedScore;
    } else if (!isNaN(parsedScore) && parsedScore > 1 && parsedScore <= 100) {
      // Handle percentage format (e.g., CONFIDENCE: 85)
      confidence = parsedScore / 100;
    }
    // Remove the confidence line from content
    content = fullContent.slice(0, confidenceMatch.index).trimEnd();
  }

  return { content, confidence };
}
