import type { ModeContext, ModeResult, MessageUsage } from "./types";
import { getContextInstances, findSpecialInstanceId } from "./types";
import type { ActiveModeState, DebateTurn } from "@/stores/streamingStore";
import type { DebateTurnData, ModelInstance } from "@/components/chat-types";
import {
  DEFAULT_DEBATE_OPENING_PROMPT,
  DEFAULT_DEBATE_REBUTTAL_PROMPT,
  DEFAULT_DEBATE_SUMMARY_PROMPT,
} from "./prompts";
import {
  aggregateUsage,
  extractUserMessageText,
  formatRoundTranscript,
  formatSingleRound,
} from "./utils";
import { defineModeSpec, runMode, type InstanceGatherResult } from "./runner";

/**
 * Debated mode state - matches the ActiveModeState variant for "debated"
 */
export type DebatedState = Extract<ActiveModeState, { mode: "debated" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface DebatedExecutionState extends DebatedState {
  _results?: Array<ModeResult | null>;
}

/**
 * Assign positions to instances for the debate.
 * By default, assigns alternating "pro" and "con" positions.
 * Positions are keyed by instance ID.
 */
function assignPositions(instances: ModelInstance[]): Record<string, string> {
  const positions: Record<string, string> = {};
  const defaultPositions = ["pro", "con"];

  instances.forEach((instance, index) => {
    positions[instance.id] = defaultPositions[index % defaultPositions.length];
  });

  return positions;
}

/**
 * Format debate transcript for summary prompt
 */
function formatDebateTranscript(turns: DebateTurn[], positions: Record<string, string>): string {
  return formatRoundTranscript(turns, {
    getRoundLabel: (round) => (round === 0 ? "Opening Statements" : `Round ${round} Rebuttals`),
    getItemLabel: (model) => positions[model],
  });
}

/**
 * Format arguments from the previous round for rebuttal prompt
 */
function formatPreviousRoundArguments(
  turns: DebateTurn[],
  round: number,
  positions: Record<string, string>
): string {
  return formatSingleRound(turns, round - 1, (model) => positions[model]);
}

/**
 * Debated mode specification.
 *
 * Flow:
 * 1. Assign positions (pro/con by default) to models
 * 2. Round 0 (Opening): Each model presents their opening argument
 * 3. Rounds 1-N (Rebuttals): Each model responds to opposing arguments
 * 4. Summary: A designated model synthesizes the debate
 *
 * The final output is the summary that considers all perspectives.
 */
export const debatedSpec = defineModeSpec<DebatedState>({
  name: "debated",
  minModels: 2, // Need at least 2 models for a debate

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    const totalRounds = ctx.modeConfig?.debateRounds ?? 3;
    // Find summarizer instance by instance ID, model ID, or fall back to first
    const summarizerInstanceId = findSpecialInstanceId(
      instances,
      ctx.modeConfig?.synthesizerInstanceId,
      ctx.modeConfig?.synthesizerModel
    );
    const summarizerInstance = instances.find((inst) => inst.id === summarizerInstanceId);
    const positions = assignPositions(instances);

    return {
      mode: "debated",
      phase: "opening",
      currentRound: 0,
      totalRounds,
      positions,
      turns: [],
      currentRoundTurns: [],
      summarizerModel: summarizerInstance?.modelId || instances[0]?.modelId,
      summarizerInstanceId: summarizerInstance?.id || instances[0]?.id,
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, streamingStore, apiContent } = ctx;

    const { totalRounds, summarizerModel, summarizerInstanceId, positions } = runner.state;

    // Get instances
    const instances = runner.getInstances();

    // Build instance lookup and helper for display names
    const instanceById = new Map<string, ModelInstance>();
    for (const inst of instances) {
      instanceById.set(inst.id, inst);
    }
    const getDisplayName = (instanceId: string): string => {
      const inst = instanceById.get(instanceId);
      return inst?.label || inst?.modelId || instanceId;
    };

    // Find summarizer instance by instance ID
    const summarizerInstance = instances.find((inst) => inst.id === summarizerInstanceId);

    // Track all turns for metadata
    const allTurns: DebateTurn[] = [];

    // Get the user message as text for prompts
    const userMessageText = extractUserMessageText(apiContent!);

    // Round 0: Opening statements (parallel) using instance-aware gathering
    const gatherResult: InstanceGatherResult = await runner.gatherInstances({
      instances,
      buildInputItems: (instance) => {
        const position = positions[instance.id];
        const openingPrompt =
          modeConfig?.debatePrompt ||
          DEFAULT_DEBATE_OPENING_PROMPT.replace(/{position}/g, position).replace(
            "{question}",
            userMessageText
          );

        return [
          ...runner.buildConversationInput(instance.modelId, apiContent!).slice(0, -1), // Get history without user message
          { role: "system", content: openingPrompt },
          { role: "user", content: "Present your opening argument." },
        ];
      },
      onInstanceComplete: (instance: ModelInstance, result) => {
        if (result) {
          const position = positions[instance.id];
          const turn: DebateTurn = {
            model: getDisplayName(instance.id),
            position,
            content: result.content,
            round: 0,
            usage: result.usage,
          };
          allTurns.push(turn);
          streamingStore.updateModeState((current) => {
            if (current.mode !== "debated") return current;
            return {
              ...current,
              turns: [...current.turns, turn],
              currentRoundTurns:
                current.currentRound === 0 ? [...current.currentRoundTurns, turn] : [turn],
            };
          });
        }
      },
    });

    // Check if we have enough responses to continue
    if (gatherResult.successfulResults.length < 2) {
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      if (gatherResult.successfulResults.length === 1) {
        const responseInstance = gatherResult.successfulResults[0].instance;
        const responseIndex = instances.findIndex((inst) => inst.id === responseInstance.id);
        if (responseIndex !== -1 && allTurns.length > 0) {
          results[responseIndex] = {
            content: allTurns[0].content,
            usage: allTurns[0].usage,
            modeMetadata: {
              mode: "debated",
              isDebateSummary: false,
              debatePositions: positions,
              debateTurns: allTurns as DebateTurnData[],
              debateRounds: 1,
            },
          };
        }
      }

      const finalState: DebatedExecutionState = {
        mode: "debated",
        phase: "done",
        currentRound: 0,
        totalRounds,
        positions,
        turns: allTurns,
        currentRoundTurns: [],
        summarizerModel,
        summarizerInstanceId,
        _results: results,
      };
      return finalState;
    }

    // Rebuttal rounds (1 to totalRounds)
    for (let round = 1; round <= totalRounds; round++) {
      // Update state to debating phase
      runner.setState({
        mode: "debated",
        phase: "debating",
        currentRound: round,
        totalRounds,
        positions,
        turns: allTurns,
        currentRoundTurns: [],
        summarizerModel,
        summarizerInstanceId,
      });

      // Build the rebuttal prompt with previous round's arguments
      const previousArguments = formatPreviousRoundArguments(allTurns, round, positions);

      // Gather rebuttals from all instances
      await runner.gatherInstances({
        instances,
        buildInputItems: (instance) => {
          const position = positions[instance.id];
          const rebuttalPrompt =
            modeConfig?.debatePrompt ||
            DEFAULT_DEBATE_REBUTTAL_PROMPT.replace(/{position}/g, position)
              .replace("{question}", userMessageText)
              .replace("{arguments}", previousArguments);

          return [
            { role: "system", content: rebuttalPrompt },
            { role: "user", content: "Provide your rebuttal." },
          ];
        },
        onInstanceComplete: (instance: ModelInstance, result) => {
          if (result) {
            const position = positions[instance.id];
            const turn: DebateTurn = {
              model: getDisplayName(instance.id),
              position,
              content: result.content,
              round,
              usage: result.usage,
            };
            allTurns.push(turn);
            streamingStore.updateModeState((current) => {
              if (current.mode !== "debated") return current;
              return {
                ...current,
                turns: [...current.turns, turn],
                currentRoundTurns:
                  current.currentRound === round ? [...current.currentRoundTurns, turn] : [turn],
              };
            });
          }
        },
      });
    }

    // Summarizing phase
    runner.setState({
      mode: "debated",
      phase: "summarizing",
      currentRound: totalRounds,
      totalRounds,
      positions,
      turns: allTurns,
      currentRoundTurns: [],
      summarizerModel,
      summarizerInstanceId,
    });

    // Build the summary prompt with full debate transcript
    const debateTranscript = formatDebateTranscript(allTurns, positions);
    const summaryPrompt = DEFAULT_DEBATE_SUMMARY_PROMPT.replace(
      "{question}",
      userMessageText
    ).replace("{debate}", debateTranscript);

    let summaryContent = "";
    let summaryUsage: MessageUsage | undefined;

    if (summarizerInstance) {
      try {
        const result = await runner.streamInstance({
          instance: summarizerInstance,
          inputItems: [
            { role: "system", content: summaryPrompt },
            { role: "user", content: "Provide a balanced summary of this debate." },
          ],
        });

        if (result) {
          summaryContent = result.content;
          summaryUsage = result.usage;
        }
      } catch {
        // If summary fails, use fallback message
        summaryContent =
          "The debate covered multiple perspectives but could not be summarized. See the debate history for details.";
      }
    } else {
      summaryContent =
        "The debate covered multiple perspectives but could not be summarized. See the debate history for details.";
    }

    // Return the summary as the result
    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
    const summarizerIndex = summarizerInstance
      ? instances.findIndex((inst) => inst.id === summarizerInstance.id)
      : -1;

    if (summarizerIndex !== -1) {
      const totalUsage = aggregateUsage(allTurns, summaryUsage);

      results[summarizerIndex] = {
        content: summaryContent,
        usage: summaryUsage,
        modeMetadata: {
          mode: "debated",
          isDebateSummary: true,
          debatePositions: positions,
          debateTurns: allTurns as DebateTurnData[],
          debateRounds: totalRounds + 1, // Include opening round
          summarizerModel: getDisplayName(summarizerInstance!.id),
          summaryUsage,
          aggregateUsage: totalUsage,
        },
      };
    }

    // Create final state
    const finalState: DebatedExecutionState = {
      mode: "debated",
      phase: "done",
      currentRound: totalRounds,
      totalRounds,
      positions,
      turns: allTurns,
      currentRoundTurns: [],
      summarizerModel,
      summarizerInstanceId,
      summary: summaryContent,
      summaryUsage,
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as DebatedExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "debated" mode - models argue different positions.
 *
 * Flow:
 * 1. Assign positions (pro/con by default) to models
 * 2. Round 0 (Opening): Each model presents their opening argument
 * 3. Rounds 1-N (Rebuttals): Each model responds to opposing arguments
 * 4. Summary: A designated model synthesizes the debate
 *
 * The final output is the summary that considers all perspectives.
 */
export async function sendDebatedMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(debatedSpec, apiContent, ctx, sendMultipleMode);
}
