import type { ModeContext, ModeResult, MessageUsage } from "./types";
import { getContextInstances } from "./types";
import type { ActiveModeState, CandidateResponse, TournamentMatch } from "@/stores/streamingStore";
import type { TournamentMatchData, ModelInstance } from "@/components/chat-types";
import { DEFAULT_TOURNAMENT_JUDGING_PROMPT } from "./prompts";
import { extractUserMessageText } from "./utils";
import { defineModeSpec, runMode, type InstanceGatherResult } from "./runner";

/**
 * Tournament mode state - matches the ActiveModeState variant for "tournament"
 */
export type TournamentState = Extract<ActiveModeState, { mode: "tournament" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface TournamentExecutionState extends TournamentState {
  _results?: Array<ModeResult | null>;
}

/**
 * Tournament mode specification.
 *
 * Flow:
 * 1. All models respond to the prompt in parallel (generating phase)
 * 2. Models are paired off into brackets (competing phase)
 * 3. A judge model compares each pair and picks a winner
 * 4. Winners advance to the next round
 * 5. Process repeats until one model remains
 *
 * Requires at least 4 models for a meaningful tournament (creates 2+ matches).
 * For odd numbers, one model gets a "bye" to the next round.
 */
export const tournamentSpec = defineModeSpec<TournamentState>({
  name: "tournament",
  minModels: 4, // Need at least 4 models for a meaningful tournament

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    const totalRounds = Math.ceil(Math.log2(instances.length));
    // Use instance IDs for bracket tracking (display names used in UI)
    const bracket: string[][] = [instances.map((inst) => inst.id)];

    return {
      mode: "tournament",
      phase: "generating",
      bracket,
      currentRound: 0,
      totalRounds,
      matches: [],
      initialResponses: [],
      eliminatedPerRound: [],
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, streamingStore, apiContent } = ctx;

    const { totalRounds, bracket } = runner.state;

    // Get instances
    const instances = runner.getInstances();

    // Build instance lookup maps
    const instanceById = new Map<string, ModelInstance>();
    for (const inst of instances) {
      instanceById.set(inst.id, inst);
    }

    // Helper to get display name for an instance ID
    const getDisplayName = (instanceId: string): string => {
      const inst = instanceById.get(instanceId);
      return inst?.label || inst?.modelId || instanceId;
    };

    // Collect initial responses from all instances in parallel
    const initialResponses: CandidateResponse[] = [];

    const gatherResult: InstanceGatherResult = await runner.gatherInstances({
      instances,
      buildInputItems: (instance) => runner.buildConversationInput(instance.modelId, apiContent!),
      onInstanceComplete: (instance: ModelInstance, result) => {
        if (result) {
          // Use instance ID as the model identifier (for tracking)
          // but store display name for UI
          const response: CandidateResponse = {
            model: instance.id,
            content: result.content,
            usage: result.usage,
          };
          initialResponses.push(response);
          streamingStore.updateModeState((current) => {
            if (current.mode !== "tournament") return current;
            return {
              ...current,
              initialResponses: [...current.initialResponses, response],
            };
          });
        }
      },
    });

    // If we don't have enough responses, return early
    if (gatherResult.successfulResults.length < 2) {
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      if (gatherResult.successfulResults.length === 1) {
        const winnerInstance = gatherResult.successfulResults[0].instance;
        const winnerIndex = instances.findIndex((inst) => inst.id === winnerInstance.id);
        if (winnerIndex !== -1) {
          results[winnerIndex] = {
            content: gatherResult.successfulResults[0].result.content,
            usage: gatherResult.successfulResults[0].result.usage,
            modeMetadata: {
              mode: "tournament",
              isTournamentWinner: true,
              tournamentWinner: getDisplayName(winnerInstance.id),
              bracket,
              matches: [],
              eliminatedPerRound: [],
            },
          };
        }
      }

      const finalState: TournamentExecutionState = {
        mode: "tournament",
        phase: "done",
        bracket,
        currentRound: 0,
        totalRounds,
        matches: [],
        initialResponses,
        eliminatedPerRound: [],
        _results: results,
      };
      return finalState;
    }

    // Get user message text for judging context
    const userMessageText = extractUserMessageText(apiContent!);

    // Build a map of instance ID -> response for quick lookup
    const responseMap = new Map<string, CandidateResponse>();
    for (const response of initialResponses) {
      responseMap.set(response.model, response); // model field contains instance ID
    }

    // Run tournament rounds (competitors are instance IDs)
    let currentCompetitors = initialResponses.map((r) => r.model);
    const allMatches: TournamentMatch[] = [];
    let currentRound = 0;
    const eliminatedPerRound: string[][] = [];
    const updatedBracket = [...bracket];

    // Update to competing phase
    runner.setState({
      mode: "tournament",
      phase: "competing",
      bracket: updatedBracket,
      currentRound,
      totalRounds,
      matches: [],
      initialResponses,
      eliminatedPerRound: [],
    });

    while (currentCompetitors.length > 1) {
      const roundWinners: string[] = [];
      const roundEliminated: string[] = [];
      const roundMatches: TournamentMatch[] = [];

      // If odd number of competitors, give first one a bye
      let startIndex = 0;
      if (currentCompetitors.length % 2 === 1) {
        roundWinners.push(currentCompetitors[0]);
        startIndex = 1;
      }

      // Pair off competitors (using instance IDs)
      for (let i = startIndex; i < currentCompetitors.length; i += 2) {
        const competitor1Id = currentCompetitors[i];
        const competitor2Id = currentCompetitors[i + 1];

        if (!competitor2Id) {
          // Shouldn't happen due to bye logic, but just in case
          roundWinners.push(competitor1Id);
          continue;
        }

        const matchId = `${currentRound}-${Math.floor((i - startIndex) / 2)}`;
        const response1 = responseMap.get(competitor1Id);
        const response2 = responseMap.get(competitor2Id);

        if (!response1 || !response2) {
          // One competitor has no response, other wins by default
          const winner = response1 ? competitor1Id : competitor2Id;
          roundWinners.push(winner);
          roundEliminated.push(response1 ? competitor2Id : competitor1Id);
          continue;
        }

        // Create match entry (use display names for UI)
        const match: TournamentMatch = {
          id: matchId,
          round: currentRound,
          competitor1: getDisplayName(competitor1Id),
          competitor2: getDisplayName(competitor2Id),
          status: "judging",
          response1: response1.content,
          response2: response2.content,
          usage1: response1.usage,
          usage2: response2.usage,
        };

        streamingStore.updateModeState((current) => {
          if (current.mode !== "tournament") return current;
          return {
            ...current,
            matches: [...current.matches, match],
            currentMatch: matchId,
          };
        });

        // Select a judge instance (use configured primaryModel or first instance not in this match)
        const judgeModelId = modeConfig?.primaryModel;
        let judgeInstance: ModelInstance | undefined;

        if (judgeModelId) {
          // Find an instance with the configured model ID
          judgeInstance = instances.find((inst) => inst.modelId === judgeModelId);
        }
        if (!judgeInstance) {
          // Find first instance not competing in this match
          judgeInstance = instances.find(
            (inst) => inst.id !== competitor1Id && inst.id !== competitor2Id
          );
        }
        if (!judgeInstance) {
          // Fallback to first instance
          judgeInstance = instances[0];
        }

        // Build judging prompt
        const judgingPrompt =
          modeConfig?.votingPrompt ||
          DEFAULT_TOURNAMENT_JUDGING_PROMPT.replace("{question}", userMessageText)
            .replace("{response_a}", response1.content)
            .replace("{response_b}", response2.content);

        let winnerId = competitor1Id; // Default to first if judging fails
        let judgeReasoning: string | undefined;
        let judgeUsage: MessageUsage | undefined;

        try {
          const judgeResult = await runner.streamInstance({
            instance: judgeInstance,
            inputItems: [
              { role: "system", content: judgingPrompt },
              { role: "user", content: "Please select the winner now." },
            ],
          });

          if (judgeResult) {
            const judgeText = judgeResult.content.trim().toUpperCase();
            judgeReasoning = judgeResult.content;
            judgeUsage = judgeResult.usage;

            // Parse the judge's decision - looking for A or B
            if (judgeText.includes("B") && !judgeText.includes("A")) {
              winnerId = competitor2Id;
            } else if (judgeText.includes("A")) {
              winnerId = competitor1Id;
            } else if (judgeText === "2") {
              winnerId = competitor2Id;
            } else if (judgeText === "1") {
              winnerId = competitor1Id;
            }
            // If neither found, default stays competitor1Id
          }
        } catch {
          // On error, default winner is competitor1Id
        }

        const loserId = winnerId === competitor1Id ? competitor2Id : competitor1Id;
        roundWinners.push(winnerId);
        roundEliminated.push(loserId);

        // Update match with results (use display names)
        const completedMatch: TournamentMatch = {
          ...match,
          status: "complete",
          winner: getDisplayName(winnerId),
          judge: getDisplayName(judgeInstance.id),
          reasoning: judgeReasoning,
          judgeUsage,
        };

        streamingStore.updateModeState((current) => {
          if (current.mode !== "tournament") return current;
          return {
            ...current,
            matches: current.matches.map((m) =>
              m.id === matchId
                ? {
                    ...m,
                    status: "complete" as const,
                    winner: getDisplayName(winnerId),
                    judge: getDisplayName(judgeInstance!.id),
                    reasoning: judgeReasoning,
                    judgeUsage,
                  }
                : m
            ),
          };
        });

        roundMatches.push(completedMatch);
      }

      // Record results (use display names for eliminated list)
      allMatches.push(...roundMatches);
      eliminatedPerRound.push(roundEliminated.map(getDisplayName));
      updatedBracket.push(roundWinners.map(getDisplayName));

      // Update state for next round
      currentCompetitors = roundWinners;
      currentRound++;

      runner.setState({
        mode: "tournament",
        phase: currentCompetitors.length === 1 ? "done" : "competing",
        bracket: updatedBracket,
        currentRound,
        totalRounds,
        matches: allMatches,
        initialResponses,
        eliminatedPerRound,
        winner: currentCompetitors.length === 1 ? getDisplayName(currentCompetitors[0]) : undefined,
      });
    }

    // Tournament complete - return the winner's response
    const tournamentWinnerId = currentCompetitors[0];
    const winnerResponse = responseMap.get(tournamentWinnerId);

    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);

    if (winnerResponse) {
      const winnerIndex = instances.findIndex((inst) => inst.id === tournamentWinnerId);
      if (winnerIndex !== -1) {
        // Convert TournamentMatch[] to TournamentMatchData[] for persistence
        const matchData: TournamentMatchData[] = allMatches.map((m) => ({
          id: m.id,
          round: m.round,
          competitor1: m.competitor1,
          competitor2: m.competitor2,
          winner: m.winner || m.competitor1,
          judge: m.judge || "",
          reasoning: m.reasoning,
          response1: m.response1 || "",
          response2: m.response2 || "",
          usage1: m.usage1,
          usage2: m.usage2,
          judgeUsage: m.judgeUsage,
        }));

        results[winnerIndex] = {
          content: winnerResponse.content,
          usage: winnerResponse.usage,
          modeMetadata: {
            mode: "tournament",
            isTournamentWinner: true,
            tournamentWinner: getDisplayName(tournamentWinnerId),
            bracket: updatedBracket,
            matches: matchData,
            eliminatedPerRound,
          },
        };
      }
    }

    // Create final state
    const finalState: TournamentExecutionState = {
      mode: "tournament",
      phase: "done",
      bracket: updatedBracket,
      currentRound,
      totalRounds,
      matches: allMatches,
      initialResponses,
      eliminatedPerRound,
      winner: getDisplayName(tournamentWinnerId),
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as TournamentExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "tournament" mode - models compete in elimination brackets
 * until a single winner remains.
 *
 * Flow:
 * 1. All models respond to the prompt in parallel (generating phase)
 * 2. Models are paired off into brackets (competing phase)
 * 3. A judge model compares each pair and picks a winner
 * 4. Winners advance to the next round
 * 5. Process repeats until one model remains
 *
 * Requires at least 4 models for a meaningful tournament (creates 2+ matches).
 * For odd numbers, one model gets a "bye" to the next round.
 */
export async function sendTournamentMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(tournamentSpec, apiContent, ctx, sendMultipleMode);
}
