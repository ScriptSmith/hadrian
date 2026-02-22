import type { ModeContext, ModeResult } from "./types";
import { getContextInstances } from "./types";
import type { ModelInstance } from "@/components/chat-types";
import type { ActiveModeState, ConsensusRound, CandidateResponse } from "@/stores/streamingStore";
import { DEFAULT_CONSENSUS_PROMPT } from "./prompts";
import { aggregateUsage, extractUserMessageText } from "./utils";
import { defineModeSpec, runMode } from "./runner";

/**
 * Consensus mode state - matches the ActiveModeState variant for "consensus"
 */
export type ConsensusState = Extract<ActiveModeState, { mode: "consensus" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface ConsensusExecutionState extends ConsensusState {
  _results?: Array<ModeResult | null>;
}

/**
 * Consensus mode specification.
 *
 * Flow:
 * 1. All models respond in parallel (initial round)
 * 2. Each model sees all responses and provides a revised response
 * 3. Repeat until consensus is reached or max rounds
 * 4. Final response is the consensus result
 *
 * Consensus is measured by comparing responses and checking if they agree
 * on key points. When agreement threshold is met, the process stops.
 */
export const consensusSpec = defineModeSpec<ConsensusState>({
  name: "consensus",
  minModels: 2, // Need at least 2 models for consensus

  initialize(ctx) {
    const maxRounds = ctx.modeConfig?.maxConsensusRounds ?? 5;
    const threshold = ctx.modeConfig?.consensusThreshold ?? 0.8;

    return {
      mode: "consensus",
      phase: "responding",
      currentRound: 0,
      maxRounds,
      threshold,
      rounds: [],
      currentRoundResponses: [],
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, settings, streamingStore, abortControllersRef, apiContent } = ctx;

    const { maxRounds, threshold } = runner.state;

    // Get instances from runner
    const instances = runner.getInstances();

    // Track all rounds for metadata
    const allRounds: ConsensusRound[] = [];

    // Get the user message as text for context
    const userMessageText = extractUserMessageText(apiContent!);

    // Round 1: Initial responses (parallel) using gatherInstances
    const initialResponses: CandidateResponse[] = [];

    // Track instance mapping for finding representative response later
    const instanceByDisplayName = new Map<string, ModelInstance>();

    const gatherResult = await runner.gatherInstances({
      instances,
      buildInputItems: (instance) => runner.buildConversationInput(instance.modelId, apiContent!),
      onInstanceComplete: (instance, result) => {
        if (result) {
          const displayName = instance.label || instance.modelId;
          instanceByDisplayName.set(displayName, instance);
          const candidate: CandidateResponse = {
            model: displayName,
            content: result.content,
            usage: result.usage,
          };
          initialResponses.push(candidate);
          streamingStore.updateModeState((current) => {
            if (current.mode !== "consensus") return current;
            return {
              ...current,
              currentRoundResponses:
                current.currentRound === 0
                  ? [...current.currentRoundResponses, candidate]
                  : [candidate],
            };
          });
        }
      },
    });

    // Record initial round
    allRounds.push({
      round: 0,
      responses: [...initialResponses],
      consensusReached: false,
    });

    // Check if we have enough responses to continue
    if (gatherResult.successfulResults.length < 2) {
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      if (initialResponses.length === 1) {
        const successfulInstance = gatherResult.successfulResults[0]?.instance;
        if (successfulInstance) {
          const responseIndex = instances.findIndex((inst) => inst.id === successfulInstance.id);
          if (responseIndex !== -1) {
            results[responseIndex] = {
              content: initialResponses[0].content,
              usage: initialResponses[0].usage,
              modeMetadata: {
                mode: "consensus",
                isConsensus: true,
                consensusRound: 0,
                totalRounds: 1,
                consensusReached: false,
                rounds: allRounds,
              },
            };
          }
        }
      }

      const finalState: ConsensusExecutionState = {
        mode: "consensus",
        phase: "done",
        currentRound: 0,
        maxRounds,
        threshold,
        rounds: allRounds,
        currentRoundResponses: [],
        _results: results,
      };
      return finalState;
    }

    // Revision rounds
    let currentResponses = initialResponses;
    let consensusReached = false;
    let finalRound = 0;

    for (let round = 1; round < maxRounds && !consensusReached; round++) {
      // Update state to revising phase
      runner.setState({
        mode: "consensus",
        phase: "revising",
        currentRound: round,
        maxRounds,
        threshold,
        rounds: allRounds,
        currentRoundResponses: [],
      });

      // Build the revision prompt with all previous responses (using display names)
      const responsesText = currentResponses
        .map((r) => `--- ${r.model} ---\n${r.content}`)
        .join("\n\n");

      const revisionPrompt =
        modeConfig?.consensusPrompt ||
        DEFAULT_CONSENSUS_PROMPT.replace("{question}", userMessageText).replace(
          "{responses}",
          responsesText
        );

      // Clear streams for new round with model mapping
      const instanceIds = instances.map((inst) => inst.id);
      const modelMap = new Map<string, string>();
      for (const inst of instances) {
        modelMap.set(inst.id, inst.modelId);
      }
      streamingStore.initStreaming(instanceIds, modelMap);

      // Create new abort controllers
      const revisionControllers = instances.map(() => new AbortController());
      abortControllersRef.current = revisionControllers;

      // Each instance revises based on all responses
      const revisedResponses: CandidateResponse[] = [];

      const revisionPromises = instances.map(async (instance, index) => {
        try {
          const displayName = instance.label || instance.modelId;

          const result = await ctx.streamResponse(
            instance.modelId,
            [
              { role: "system", content: revisionPrompt },
              { role: "user", content: "Please provide your revised response." },
            ],
            revisionControllers[index],
            settings,
            instance.id, // Use instance ID for streaming
            undefined,
            undefined,
            instance.parameters, // Pass instance parameters
            instance.label
          );

          if (result) {
            instanceByDisplayName.set(displayName, instance);
            const candidate: CandidateResponse = {
              model: displayName,
              content: result.content,
              usage: result.usage,
            };
            revisedResponses.push(candidate);
            streamingStore.updateModeState((current) => {
              if (current.mode !== "consensus") return current;
              return {
                ...current,
                currentRoundResponses:
                  current.currentRound === round
                    ? [...current.currentRoundResponses, candidate]
                    : [candidate],
              };
            });
          }
        } catch {
          // Ignore revision errors
        }
      });

      await Promise.all(revisionPromises);

      // Calculate consensus score for this round
      const consensusScore = calculateConsensusScore(revisedResponses);
      consensusReached = consensusScore >= threshold;

      // Record this round
      allRounds.push({
        round,
        responses: [...revisedResponses],
        consensusReached,
        consensusScore,
      });

      currentResponses = revisedResponses.length > 0 ? revisedResponses : currentResponses;
      finalRound = round;

      // Update state with current consensus check result
      runner.setState({
        mode: "consensus",
        phase: consensusReached ? "done" : "revising",
        currentRound: round,
        maxRounds,
        threshold,
        rounds: allRounds,
        currentRoundResponses: [],
        finalScore: consensusReached ? consensusScore : undefined,
      });
    }

    // Mark as done
    const finalScore = calculateConsensusScore(currentResponses);

    // Return the consensus result
    // We use the most representative response as the "consensus" response
    // but include all rounds in metadata for transparency
    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);

    // Find the most representative response (the one most similar to others)
    const representativeResponse = findRepresentativeResponse(currentResponses);

    if (representativeResponse) {
      // Find the instance that produced this response
      const representativeInstance = instanceByDisplayName.get(representativeResponse.model);
      if (representativeInstance) {
        const responseIndex = instances.findIndex((inst) => inst.id === representativeInstance.id);
        if (responseIndex !== -1) {
          // Aggregate usage from all rounds
          const totalUsage = aggregateAllUsage(allRounds);

          results[responseIndex] = {
            content: representativeResponse.content,
            usage: representativeResponse.usage,
            modeMetadata: {
              mode: "consensus",
              isConsensus: true,
              consensusRound: finalRound,
              totalRounds: allRounds.length,
              consensusReached,
              consensusScore: finalScore,
              rounds: allRounds,
              aggregateUsage: totalUsage,
            },
          };
        }
      }
    }

    // Create final state
    const finalState: ConsensusExecutionState = {
      mode: "consensus",
      phase: "done",
      currentRound: finalRound,
      maxRounds,
      threshold,
      rounds: allRounds,
      currentRoundResponses: [],
      finalScore,
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as ConsensusExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "consensus" mode - models revise their responses until they agree.
 *
 * Flow:
 * 1. All models respond in parallel (initial round)
 * 2. Each model sees all responses and provides a revised response
 * 3. Repeat until consensus is reached or max rounds
 * 4. Final response is the consensus result
 *
 * Consensus is measured by comparing responses and checking if they agree
 * on key points. When agreement threshold is met, the process stops.
 */
export async function sendConsensusMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(consensusSpec, apiContent, ctx, sendMultipleMode);
}

/**
 * Calculate a consensus score based on response similarity.
 * Returns a value between 0 and 1, where 1 means perfect agreement.
 *
 * This uses a simple approach:
 * 1. Tokenize each response into words
 * 2. Calculate Jaccard similarity between all pairs
 * 3. Return the average similarity
 */
function calculateConsensusScore(responses: CandidateResponse[]): number {
  if (responses.length < 2) return 1.0;

  const tokenizedResponses = responses.map((r) => tokenize(r.content));
  let totalSimilarity = 0;
  let pairCount = 0;

  for (let i = 0; i < tokenizedResponses.length; i++) {
    for (let j = i + 1; j < tokenizedResponses.length; j++) {
      totalSimilarity += jaccardSimilarity(tokenizedResponses[i], tokenizedResponses[j]);
      pairCount++;
    }
  }

  return pairCount > 0 ? totalSimilarity / pairCount : 1.0;
}

/**
 * Tokenize text into lowercase words, removing punctuation
 */
function tokenize(text: string): Set<string> {
  return new Set(
    text
      .toLowerCase()
      .replace(/[^\w\s]/g, " ")
      .split(/\s+/)
      .filter((w) => w.length > 2) // Filter out very short words
  );
}

/**
 * Calculate Jaccard similarity between two sets
 */
function jaccardSimilarity(set1: Set<string>, set2: Set<string>): number {
  const intersection = new Set([...set1].filter((x) => set2.has(x)));
  const union = new Set([...set1, ...set2]);
  return union.size > 0 ? intersection.size / union.size : 1.0;
}

/**
 * Find the most representative response (highest average similarity to others)
 */
function findRepresentativeResponse(responses: CandidateResponse[]): CandidateResponse | undefined {
  if (responses.length === 0) return undefined;
  if (responses.length === 1) return responses[0];

  const tokenizedResponses = responses.map((r) => tokenize(r.content));
  let bestIndex = 0;
  let bestAvgSimilarity = 0;

  for (let i = 0; i < responses.length; i++) {
    let totalSimilarity = 0;
    for (let j = 0; j < responses.length; j++) {
      if (i !== j) {
        totalSimilarity += jaccardSimilarity(tokenizedResponses[i], tokenizedResponses[j]);
      }
    }
    const avgSimilarity = totalSimilarity / (responses.length - 1);
    if (avgSimilarity > bestAvgSimilarity) {
      bestAvgSimilarity = avgSimilarity;
      bestIndex = i;
    }
  }

  return responses[bestIndex];
}

/**
 * Aggregate usage from all rounds by flattening all responses
 */
function aggregateAllUsage(rounds: ConsensusRound[]) {
  const allResponses = rounds.flatMap((round) => round.responses);
  return aggregateUsage(allResponses);
}
