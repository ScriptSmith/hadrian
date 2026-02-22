import type { ModeContext, ModeResult, MessageUsage } from "./types";
import { getContextInstances } from "./types";
import type { ActiveModeState, CandidateResponse, VoteData } from "@/stores/streamingStore";
import { DEFAULT_VOTING_PROMPT } from "./prompts";
import { extractUserMessageText } from "./utils";
import { defineModeSpec, runMode } from "./runner";

/**
 * Elected mode state - matches the ActiveModeState variant for "elected"
 */
export type ElectedState = Extract<ActiveModeState, { mode: "elected" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface ElectedExecutionState extends ElectedState {
  _results?: Array<ModeResult | null>;
}

/**
 * Aggregate usage from all votes
 */
function aggregateVoteUsage(votes: VoteData[]): MessageUsage {
  return votes.reduce(
    (acc, vote) => ({
      inputTokens: acc.inputTokens + (vote.usage?.inputTokens ?? 0),
      outputTokens: acc.outputTokens + (vote.usage?.outputTokens ?? 0),
      totalTokens: acc.totalTokens + (vote.usage?.totalTokens ?? 0),
      cost: (acc.cost ?? 0) + (vote.usage?.cost ?? 0),
    }),
    { inputTokens: 0, outputTokens: 0, totalTokens: 0, cost: 0 }
  );
}

/**
 * Elected mode specification.
 *
 * Flow:
 * 1. All models respond in parallel (responding phase)
 * 2. All models vote on which response is best (voting phase)
 * 3. The response with the most votes wins
 *
 * Requires at least 3 models to be meaningful (2 candidates + 1 voter at minimum)
 */
export const electedSpec = defineModeSpec<ElectedState>({
  name: "elected",
  minModels: 3, // Need at least 3 instances for a meaningful election

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    return {
      mode: "elected",
      phase: "responding",
      candidates: [],
      completedResponses: 0,
      totalModels: instances.length,
      votes: [],
      completedVotes: 0,
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, settings, streamingStore, abortControllersRef, apiContent } = ctx;

    // Get instances from runner
    const instances = runner.getInstances();

    // Collect candidate responses using gatherInstances
    const candidates: CandidateResponse[] = [];

    const gatherResult = await runner.gatherInstances({
      instances,
      buildInputItems: (instance) => runner.buildConversationInput(instance.modelId, apiContent!),
      onInstanceComplete: (instance, result) => {
        if (result) {
          // Use instance label for display, but track by model ID for compatibility
          const displayName = instance.label || instance.modelId;
          const candidate: CandidateResponse = {
            model: displayName,
            content: result.content,
            usage: result.usage,
          };
          candidates.push(candidate);
          runner.updateState((current) => ({
            ...current,
            candidates: [...current.candidates, candidate],
            completedResponses: current.completedResponses + 1,
          }));
        }
      },
    });

    // If we don't have enough candidates to vote on, return what we have
    if (gatherResult.successfulResults.length < 2) {
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      if (candidates.length === 1) {
        // Find the instance that succeeded
        const successfulInstance = gatherResult.successfulResults[0]?.instance;
        if (successfulInstance) {
          const winnerIndex = instances.findIndex((inst) => inst.id === successfulInstance.id);
          if (winnerIndex !== -1) {
            results[winnerIndex] = {
              content: candidates[0].content,
              usage: candidates[0].usage,
              modeMetadata: {
                mode: "elected",
                isElected: true,
                winner: candidates[0].model,
                candidates,
                votes: [],
                voteCounts: { [candidates[0].model]: 0 },
              },
            };
          }
        }
      }

      const finalState: ElectedExecutionState = {
        mode: "elected",
        phase: "done",
        candidates,
        completedResponses: candidates.length,
        totalModels: instances.length,
        votes: [],
        completedVotes: 0,
        _results: results,
      };
      return finalState;
    }

    // Update state to voting phase
    runner.setState({
      mode: "elected",
      phase: "voting",
      candidates,
      completedResponses: candidates.length,
      totalModels: instances.length,
      votes: [],
      completedVotes: 0,
    });

    // Get the user message as text for voting context
    const userMessageText = extractUserMessageText(apiContent!);

    // Build the candidates text for voting (using display names)
    const candidatesText = candidates
      .map((c, idx) => `--- Candidate ${idx + 1} (${c.model}) ---\n${c.content}`)
      .join("\n\n");

    const votingPrompt =
      modeConfig?.votingPrompt ||
      DEFAULT_VOTING_PROMPT.replace("{question}", userMessageText).replace(
        "{candidates}",
        candidatesText
      );

    // Collect votes from all instances
    const votes: VoteData[] = [];
    const voteControllers = instances.map(() => new AbortController());
    abortControllersRef.current = voteControllers;

    // Each instance votes
    // Note: We use a higher maxTokens (150) to accommodate reasoning models that use
    // tokens for internal reasoning before producing output. 10 tokens was too restrictive.
    const votePromises = instances.map(async (instance, index) => {
      try {
        // Initialize streaming for the voter with model mapping
        const modelMap = new Map<string, string>();
        modelMap.set(instance.id, instance.modelId);
        streamingStore.initStreaming([instance.id], modelMap);

        // Use instance label for voter identification
        const voterName = instance.label || instance.modelId;

        const voteResult = await ctx.streamResponse(
          instance.modelId,
          [
            { role: "system", content: votingPrompt },
            { role: "user", content: "Please cast your vote now." },
          ],
          voteControllers[index],
          { ...settings, maxTokens: 150 }, // Allow enough tokens for reasoning models
          instance.id, // Use instance ID for streaming
          undefined,
          undefined,
          instance.parameters, // Pass instance parameters
          instance.label
        );

        if (voteResult) {
          // Parse the vote - expecting a single number somewhere in the response
          const voteText = voteResult.content.trim();
          const voteNumber = parseInt(voteText.match(/\d+/)?.[0] || "0", 10);

          const candidateIndex = voteNumber - 1;
          if (candidateIndex >= 0 && candidateIndex < candidates.length) {
            const votedFor = candidates[candidateIndex].model;
            // Allow self-votes - models may legitimately think their response is best
            const vote: VoteData = {
              voter: voterName,
              votedFor,
              reasoning: voteText,
              usage: voteResult.usage,
            };
            votes.push(vote);
            runner.updateState((current) => ({
              ...current,
              votes: [...current.votes, vote],
              completedVotes: current.completedVotes + 1,
            }));
          }
        }
      } catch {
        // Ignore vote errors - some instances might fail to vote
      }
    });

    await Promise.all(votePromises);

    // Count votes (by candidate display name)
    const voteCounts: Record<string, number> = {};
    for (const candidate of candidates) {
      voteCounts[candidate.model] = 0;
    }
    for (const vote of votes) {
      if (voteCounts[vote.votedFor] !== undefined) {
        voteCounts[vote.votedFor]++;
      }
    }

    // Determine winner (most votes wins, tie goes to first alphabetically for determinism)
    let winner = candidates[0].model;
    let maxVotes = voteCounts[winner] || 0;
    for (const candidate of candidates) {
      const count = voteCounts[candidate.model] || 0;
      if (count > maxVotes || (count === maxVotes && candidate.model < winner)) {
        winner = candidate.model;
        maxVotes = count;
      }
    }

    // Build final results - find the instance index for the winner
    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
    const winnerCandidate = candidates.find((c) => c.model === winner);
    if (winnerCandidate) {
      // Find the instance that produced this candidate
      // Match by looking at successful results
      const winnerResult = gatherResult.successfulResults.find((sr) => {
        const displayName = sr.instance.label || sr.instance.modelId;
        return displayName === winner;
      });
      if (winnerResult) {
        const winnerIndex = instances.findIndex((inst) => inst.id === winnerResult.instance.id);
        if (winnerIndex !== -1) {
          results[winnerIndex] = {
            content: winnerCandidate.content,
            usage: winnerCandidate.usage,
            modeMetadata: {
              mode: "elected",
              isElected: true,
              winner,
              candidates,
              votes,
              voteCounts,
              voteUsage: aggregateVoteUsage(votes),
            },
          };
        }
      }
    }

    // Create final state
    const finalState: ElectedExecutionState = {
      mode: "elected",
      phase: "done",
      candidates,
      completedResponses: candidates.length,
      totalModels: instances.length,
      votes,
      completedVotes: votes.length,
      winner,
      voteCounts,
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as ElectedExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "elected" mode - all models respond in parallel,
 * then all models vote on the best response.
 *
 * Flow:
 * 1. All models respond in parallel (responding phase)
 * 2. All models vote on which response is best (voting phase)
 * 3. The response with the most votes wins
 *
 * Requires at least 3 models to be meaningful (2 candidates + 1 voter at minimum)
 */
export async function sendElectedMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(electedSpec, apiContent, ctx, sendMultipleMode);
}
