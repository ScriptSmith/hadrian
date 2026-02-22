import type { ModeContext, ModeResult, MessageUsage } from "./types";
import { getContextInstances, findSpecialInstanceId } from "./types";
import type { ActiveModeState, CouncilStatement } from "@/stores/streamingStore";
import type { CouncilStatementData, ModelInstance } from "@/components/chat-types";
import {
  DEFAULT_COUNCIL_OPENING_PROMPT,
  DEFAULT_COUNCIL_DISCUSSION_PROMPT,
  DEFAULT_COUNCIL_SYNTHESIS_PROMPT,
  DEFAULT_COUNCIL_ROLE_ASSIGNMENT_PROMPT,
} from "./prompts";
import {
  aggregateUsage,
  extractUserMessageText,
  formatRoundTranscript,
  formatSingleRound,
  getShortModelName,
  parseJsonFromResponse,
} from "./utils";
import { defineModeSpec, runMode, type InstanceGatherResult } from "./runner";

/**
 * Council mode state - matches the ActiveModeState variant for "council"
 */
export type CouncilState = Extract<ActiveModeState, { mode: "council" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface CouncilExecutionState extends CouncilState {
  _results?: Array<ModeResult | null>;
}

/**
 * Default roles for council mode when not specified
 */
const DEFAULT_COUNCIL_ROLES = [
  "Technical Expert",
  "Business Analyst",
  "User Advocate",
  "Risk Assessor",
  "Innovation Specialist",
  "Quality Assurance",
  "Operations Lead",
  "Strategy Advisor",
];

/**
 * Assign roles to instances for the council discussion.
 * Uses configured roles if available (matching by instance ID or model ID),
 * otherwise assigns from default list.
 * Roles are keyed by instance ID.
 */
function assignRoles(
  instances: ModelInstance[],
  configuredRoles?: Record<string, string>
): Record<string, string> {
  const roles: Record<string, string> = {};

  instances.forEach((instance, index) => {
    // Use configured role if available (try instance ID first, then model ID)
    roles[instance.id] =
      configuredRoles?.[instance.id] ||
      configuredRoles?.[instance.modelId] ||
      DEFAULT_COUNCIL_ROLES[index % DEFAULT_COUNCIL_ROLES.length];
  });

  return roles;
}

/**
 * Parse auto-assigned roles from the model response.
 * Expects a JSON object mapping model names to roles.
 * Returns roles keyed by instance ID.
 */
function parseAssignedRoles(
  response: string,
  councilMembers: ModelInstance[]
): Record<string, string> | null {
  const parsed = parseJsonFromResponse<Record<string, string>>(response);
  if (!parsed) return null;

  // Verify we have roles for all council members
  const roles: Record<string, string> = {};
  for (const instance of councilMembers) {
    // Try exact match first (instance ID or model ID), then partial match on model name
    const shortModel = getShortModelName(instance.modelId);
    const role =
      parsed[instance.id] ||
      parsed[instance.modelId] ||
      parsed[shortModel] ||
      Object.entries(parsed).find(
        ([key]) => key.includes(shortModel) || shortModel.includes(key)
      )?.[1];

    if (role && typeof role === "string") {
      roles[instance.id] = role;
    }
  }

  // Return null if we didn't get roles for all members
  if (Object.keys(roles).length !== councilMembers.length) {
    return null;
  }

  return roles;
}

/**
 * Format council discussion transcript for synthesis prompt
 */
function formatCouncilTranscript(
  statements: CouncilStatement[],
  roles: Record<string, string>
): string {
  return formatRoundTranscript(statements, {
    getRoundLabel: (round) => (round === 0 ? "Opening Perspectives" : `Discussion Round ${round}`),
    getItemLabel: (model) => roles[model],
  });
}

/**
 * Format previous round's perspectives for discussion prompt
 */
function formatPreviousRoundPerspectives(
  statements: CouncilStatement[],
  round: number,
  roles: Record<string, string>
): string {
  return formatSingleRound(statements, round - 1, (model) => roles[model]);
}

/**
 * Council mode specification.
 *
 * Flow:
 * 1. (Optional) Auto-assign roles using the synthesizer model
 * 2. Round 0 (Opening): Each council member presents their initial perspective from their role
 * 3. Rounds 1-N (Discussion): Each council member responds to other perspectives
 * 4. Synthesis: The synthesizer model (which does NOT participate in the council) synthesizes all perspectives
 *
 * The synthesizer model is separate from the council members and only observes and synthesizes.
 */
export const councilSpec = defineModeSpec<CouncilState>({
  name: "council",
  minModels: 2, // Need at least 2 models (1 synthesizer + 1 council member)

  validate(ctx) {
    // Need at least 1 council member (separate from synthesizer)
    const instances = getContextInstances(ctx);
    const synthesizerInstanceId = findSpecialInstanceId(
      instances,
      ctx.modeConfig?.synthesizerInstanceId,
      ctx.modeConfig?.synthesizerModel
    );
    const councilMembers = instances.filter((inst) => inst.id !== synthesizerInstanceId);
    return councilMembers.length > 0;
  },

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    const totalRounds = ctx.modeConfig?.debateRounds ?? 2;
    // Find synthesizer instance by instance ID, model ID, or fall back to first
    const synthesizerInstanceId = findSpecialInstanceId(
      instances,
      ctx.modeConfig?.synthesizerInstanceId,
      ctx.modeConfig?.synthesizerModel
    );
    const synthesizerInstance = instances.find((inst) => inst.id === synthesizerInstanceId);
    const autoAssignRoles = ctx.modeConfig?.councilAutoAssignRoles ?? false;

    // Council members are all instances EXCEPT the synthesizer
    const councilMembers = instances.filter((inst) => inst.id !== synthesizerInstanceId);

    // If auto-assign, start with assigning phase and empty roles
    // Otherwise, assign roles immediately (keyed by instance ID)
    const initialPhase = autoAssignRoles ? "assigning" : "opening";
    const initialRoles = autoAssignRoles
      ? {}
      : assignRoles(councilMembers, ctx.modeConfig?.councilRoles);

    return {
      mode: "council",
      phase: initialPhase,
      currentRound: 0,
      totalRounds,
      roles: initialRoles,
      statements: [],
      currentRoundStatements: [],
      synthesizerModel: synthesizerInstance?.modelId || instances[0]?.modelId,
      synthesizerInstanceId: synthesizerInstance?.id || instances[0]?.id,
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, streamingStore, apiContent } = ctx;

    const {
      totalRounds,
      synthesizerModel,
      synthesizerInstanceId,
      roles: initialRoles,
      phase: initialPhase,
    } = runner.state;
    const autoAssignRoles = modeConfig?.councilAutoAssignRoles ?? false;

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

    // Council members are all instances EXCEPT the synthesizer (matched by instance ID)
    const councilMembers = instances.filter((inst) => inst.id !== synthesizerInstanceId);

    // Find synthesizer instance by instance ID
    const synthesizerInstance = instances.find((inst) => inst.id === synthesizerInstanceId);

    // Get the user message as text for prompts
    const userMessageText = extractUserMessageText(apiContent!);

    // Track all statements for metadata
    const allStatements: CouncilStatement[] = [];
    let roleAssignmentUsage: MessageUsage | undefined;

    // Get roles - either from initial state or via auto-assignment
    let roles: Record<string, string> = initialRoles;

    if (autoAssignRoles && initialPhase === "assigning" && synthesizerInstance) {
      // Auto-assign roles using the synthesizer model
      const membersList = councilMembers
        .map((inst) => `- ${getShortModelName(inst.modelId)}`)
        .join("\n");
      const assignPrompt = DEFAULT_COUNCIL_ROLE_ASSIGNMENT_PROMPT.replace(
        "{question}",
        userMessageText
      )
        .replace("{count}", councilMembers.length.toString())
        .replace("{members}", membersList);

      try {
        const assignResult = await runner.streamInstance({
          instance: synthesizerInstance,
          inputItems: [
            { role: "system", content: assignPrompt },
            { role: "user", content: "Assign roles to the council members." },
          ],
        });

        if (assignResult) {
          roleAssignmentUsage = assignResult.usage;
          const parsedRoles = parseAssignedRoles(assignResult.content, councilMembers);
          if (parsedRoles) {
            roles = parsedRoles;
          } else {
            // Fallback to default roles if parsing failed
            roles = assignRoles(councilMembers, modeConfig?.councilRoles);
          }
        } else {
          roles = assignRoles(councilMembers, modeConfig?.councilRoles);
        }
      } catch {
        // Fallback to default roles on error
        roles = assignRoles(councilMembers, modeConfig?.councilRoles);
      }

      // Update state with assigned roles and move to opening phase
      runner.setState({
        mode: "council",
        phase: "opening",
        currentRound: 0,
        totalRounds,
        roles,
        statements: [],
        currentRoundStatements: [],
        synthesizerModel,
        synthesizerInstanceId,
      });
    }

    // Round 0: Opening perspectives (parallel) - council members only
    const gatherResult: InstanceGatherResult = await runner.gatherInstances({
      instances: councilMembers,
      buildInputItems: (instance) => {
        const role = roles[instance.id];
        const openingPrompt =
          modeConfig?.councilPrompt ||
          DEFAULT_COUNCIL_OPENING_PROMPT.replace(/{role}/g, role).replace(
            "{question}",
            userMessageText
          );

        return [
          ...runner.buildConversationInput(instance.modelId, apiContent!).slice(0, -1), // Get history without user message
          { role: "system", content: openingPrompt },
          { role: "user", content: "Present your initial perspective." },
        ];
      },
      onInstanceComplete: (instance: ModelInstance, result) => {
        if (result) {
          const role = roles[instance.id];
          const statement: CouncilStatement = {
            model: getDisplayName(instance.id),
            role,
            content: result.content,
            round: 0,
            usage: result.usage,
          };
          allStatements.push(statement);
          streamingStore.updateModeState((current) => {
            if (current.mode !== "council") return current;
            return {
              ...current,
              statements: [...current.statements, statement],
              currentRoundStatements:
                current.currentRound === 0
                  ? [...current.currentRoundStatements, statement]
                  : [statement],
            };
          });
        }
      },
    });

    // Check if we have enough responses to continue
    if (gatherResult.successfulResults.length < 1) {
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      const finalState: CouncilExecutionState = {
        mode: "council",
        phase: "done",
        currentRound: 0,
        totalRounds,
        roles,
        statements: allStatements,
        currentRoundStatements: [],
        synthesizerModel,
        synthesizerInstanceId,
        _results: results,
      };
      return finalState;
    }

    // Discussion rounds (1 to totalRounds) - council members only
    for (let round = 1; round <= totalRounds; round++) {
      // Update state to discussing phase
      runner.setState({
        mode: "council",
        phase: "discussing",
        currentRound: round,
        totalRounds,
        roles,
        statements: allStatements,
        currentRoundStatements: [],
        synthesizerModel,
        synthesizerInstanceId,
      });

      // Build the discussion prompt with previous round's perspectives
      const previousPerspectives = formatPreviousRoundPerspectives(allStatements, round, roles);

      // Gather discussion responses from all council members
      await runner.gatherInstances({
        instances: councilMembers,
        buildInputItems: (instance) => {
          const role = roles[instance.id];
          const discussionPrompt =
            modeConfig?.councilPrompt ||
            DEFAULT_COUNCIL_DISCUSSION_PROMPT.replace(/{role}/g, role)
              .replace("{question}", userMessageText)
              .replace("{perspectives}", previousPerspectives);

          return [
            { role: "system", content: discussionPrompt },
            { role: "user", content: "Provide your response to the other perspectives." },
          ];
        },
        onInstanceComplete: (instance: ModelInstance, result) => {
          if (result) {
            const role = roles[instance.id];
            const statement: CouncilStatement = {
              model: getDisplayName(instance.id),
              role,
              content: result.content,
              round,
              usage: result.usage,
            };
            allStatements.push(statement);
            streamingStore.updateModeState((current) => {
              if (current.mode !== "council") return current;
              return {
                ...current,
                statements: [...current.statements, statement],
                currentRoundStatements:
                  current.currentRound === round
                    ? [...current.currentRoundStatements, statement]
                    : [statement],
              };
            });
          }
        },
      });
    }

    // Synthesizing phase - synthesizer model only (it did NOT participate in the council)
    runner.setState({
      mode: "council",
      phase: "synthesizing",
      currentRound: totalRounds,
      totalRounds,
      roles,
      statements: allStatements,
      currentRoundStatements: [],
      synthesizerModel,
      synthesizerInstanceId,
    });

    // Build the synthesis prompt with full council transcript
    const councilTranscript = formatCouncilTranscript(allStatements, roles);
    const synthesisPrompt = DEFAULT_COUNCIL_SYNTHESIS_PROMPT.replace(
      "{question}",
      userMessageText
    ).replace("{discussion}", councilTranscript);

    let synthesisContent = "";
    let synthesisUsage: MessageUsage | undefined;

    if (synthesizerInstance) {
      try {
        const result = await runner.streamInstance({
          instance: synthesizerInstance,
          inputItems: [
            { role: "system", content: synthesisPrompt },
            {
              role: "user",
              content: "Provide a comprehensive synthesis of this council discussion.",
            },
          ],
        });

        if (result) {
          synthesisContent = result.content;
          synthesisUsage = result.usage;
        }
      } catch {
        // If synthesis fails, provide fallback message
        synthesisContent =
          "The council discussed multiple perspectives but could not synthesize them. See the discussion history for details.";
      }
    } else {
      synthesisContent =
        "The council discussed multiple perspectives but could not synthesize them. See the discussion history for details.";
    }

    // Return the synthesis as the result (on the synthesizer instance's slot)
    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
    const synthesizerIndex = synthesizerInstance
      ? instances.findIndex((inst) => inst.id === synthesizerInstance.id)
      : -1;

    if (synthesizerIndex !== -1) {
      // Combine all usage: role assignment (if any) + statements + synthesis
      const totalUsage = aggregateUsage(allStatements, synthesisUsage, roleAssignmentUsage);

      results[synthesizerIndex] = {
        content: synthesisContent,
        usage: synthesisUsage,
        modeMetadata: {
          mode: "council",
          isCouncilSynthesis: true,
          councilRoles: roles,
          councilStatements: allStatements as CouncilStatementData[],
          councilRounds: totalRounds + 1, // Include opening round
          summarizerModel: getDisplayName(synthesizerInstance!.id),
          summaryUsage: synthesisUsage,
          aggregateUsage: totalUsage,
        },
      };
    }

    // Create final state
    const finalState: CouncilExecutionState = {
      mode: "council",
      phase: "done",
      currentRound: totalRounds,
      totalRounds,
      roles,
      statements: allStatements,
      currentRoundStatements: [],
      synthesizerModel,
      synthesizerInstanceId,
      synthesis: synthesisContent,
      synthesisUsage,
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as CouncilExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "council" mode - models discuss from assigned perspectives.
 *
 * Flow:
 * 1. (Optional) Auto-assign roles using the synthesizer model
 * 2. Round 0 (Opening): Each council member presents their initial perspective from their role
 * 3. Rounds 1-N (Discussion): Each council member responds to other perspectives
 * 4. Synthesis: The synthesizer model (which does NOT participate in the council) synthesizes all perspectives
 *
 * The synthesizer model is separate from the council members and only observes and synthesizes.
 */
export async function sendCouncilMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(councilSpec, apiContent, ctx, sendMultipleMode);
}
