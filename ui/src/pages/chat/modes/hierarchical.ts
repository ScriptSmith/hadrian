import type { ModeContext, ModeResult, MessageUsage } from "./types";
import { getContextInstances, findSpecialInstanceId } from "./types";
import type {
  ActiveModeState,
  HierarchicalSubtask,
  HierarchicalWorkerResult,
} from "@/stores/streamingStore";
import type { ModelInstance } from "@/components/chat-types";
import {
  DEFAULT_HIERARCHICAL_DECOMPOSITION_PROMPT,
  DEFAULT_HIERARCHICAL_WORKER_PROMPT,
  DEFAULT_HIERARCHICAL_SYNTHESIS_PROMPT,
} from "./prompts";
import {
  aggregateUsage,
  extractUserMessageText,
  getShortModelName,
  parseJsonFromResponse,
} from "./utils";
import { defineModeSpec, runMode } from "./runner";

/**
 * Hierarchical mode state - matches the ActiveModeState variant for "hierarchical"
 */
export type HierarchicalState = Extract<ActiveModeState, { mode: "hierarchical" }>;

/**
 * Extended state for tracking execution results (internal use)
 */
interface HierarchicalExecutionState extends HierarchicalState {
  _results?: Array<ModeResult | null>;
}

/** Shape of the parsed subtask decomposition response */
interface SubtaskDecomposition {
  subtasks?: Array<{ id?: string; description?: string; assignedModel?: string }>;
}

/**
 * Parse the coordinator's subtask decomposition response.
 * Expects JSON with structure: { "subtasks": [{ "id": string, "description": string, "assignedModel"?: string }] }
 * Returns subtasks with assignedModel set to instance ID.
 */
function parseSubtasks(
  response: string,
  workerInstances: ModelInstance[],
  getDisplayName: (instanceId: string) => string
): HierarchicalSubtask[] | null {
  const parsed = parseJsonFromResponse<SubtaskDecomposition>(response);
  if (!parsed?.subtasks || !Array.isArray(parsed.subtasks)) return null;

  const subtasks: HierarchicalSubtask[] = parsed.subtasks
    .filter((s) => s.description)
    .map((s, index) => {
      // Try to find matching instance by instance ID, model ID, or partial name
      let assignedInstance: ModelInstance | undefined;

      if (s.assignedModel) {
        // Try exact match by instance ID first
        assignedInstance = workerInstances.find((inst) => inst.id === s.assignedModel);

        // Try exact match by model ID
        if (!assignedInstance) {
          assignedInstance = workerInstances.find((inst) => inst.modelId === s.assignedModel);
        }

        // Try partial match on model name
        if (!assignedInstance) {
          assignedInstance = workerInstances.find((inst) => {
            const shortModel = getShortModelName(inst.modelId);
            return s.assignedModel!.includes(shortModel) || shortModel.includes(s.assignedModel!);
          });
        }
      }

      // Fallback: round-robin assignment
      if (!assignedInstance) {
        assignedInstance = workerInstances[index % workerInstances.length];
      }

      return {
        id: s.id || `subtask-${index + 1}`,
        description: s.description!,
        assignedModel: getDisplayName(assignedInstance.id),
        assignedInstanceId: assignedInstance.id,
        status: "pending" as const,
      };
    });

  return subtasks.length > 0 ? subtasks : null;
}

/**
 * Format worker results for the synthesis prompt
 */
function formatWorkerResults(results: HierarchicalWorkerResult[]): string {
  return results
    .map((result) => {
      const shortModel = getShortModelName(result.model);
      return `### Subtask: ${result.subtaskId}\n**Model:** ${shortModel}\n**Task:** ${result.description}\n\n**Result:**\n${result.content}`;
    })
    .join("\n\n---\n\n");
}

/**
 * Hierarchical mode specification.
 *
 * Flow:
 * 1. Coordinator analyzes the prompt and decomposes it into subtasks
 * 2. Subtasks are assigned to worker models (parallel execution)
 * 3. Workers complete their assigned subtasks
 * 4. Coordinator synthesizes all results into a final response
 *
 * The coordinator does NOT participate as a worker.
 */
export const hierarchicalSpec = defineModeSpec<HierarchicalState>({
  name: "hierarchical",
  minModels: 2, // Need at least 2 models (1 coordinator + 1 worker)

  validate(ctx) {
    // Need at least 1 worker (separate from coordinator)
    const instances = getContextInstances(ctx);
    const coordinatorInstanceId = findSpecialInstanceId(
      instances,
      ctx.modeConfig?.coordinatorInstanceId,
      ctx.modeConfig?.coordinatorModel
    );
    const workerInstances = instances.filter((inst) => inst.id !== coordinatorInstanceId);
    return workerInstances.length > 0;
  },

  initialize(ctx) {
    const instances = getContextInstances(ctx);
    // Find coordinator instance by instance ID, model ID, or fall back to first
    const coordinatorInstanceId = findSpecialInstanceId(
      instances,
      ctx.modeConfig?.coordinatorInstanceId,
      ctx.modeConfig?.coordinatorModel
    );
    const coordinatorInstance = instances.find((inst) => inst.id === coordinatorInstanceId);

    return {
      mode: "hierarchical",
      phase: "decomposing",
      coordinatorModel: coordinatorInstance?.modelId || instances[0]?.modelId,
      coordinatorInstanceId: coordinatorInstance?.id || instances[0]?.id,
      subtasks: [],
      workerResults: [],
    };
  },

  async execute(ctx, runner) {
    const { modeConfig, streamingStore, apiContent } = ctx;

    const { coordinatorModel, coordinatorInstanceId } = runner.state;

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

    // Find coordinator and worker instances by instance ID
    const coordinatorInstance = instances.find((inst) => inst.id === coordinatorInstanceId);
    const workerInstances = instances.filter((inst) => inst.id !== coordinatorInstanceId);

    // Get the user message as text for prompts
    const userMessageText = extractUserMessageText(apiContent!);

    // Track all worker results
    const workerResults: HierarchicalWorkerResult[] = [];
    let decompositionUsage: MessageUsage | undefined;

    // Phase 1: Decomposition - Coordinator breaks down the task
    // Build the decomposition prompt
    const workersList = workerInstances
      .map((inst) => `- ${getShortModelName(inst.modelId)}`)
      .join("\n");
    const decompositionPrompt = (
      modeConfig?.routingPrompt || DEFAULT_HIERARCHICAL_DECOMPOSITION_PROMPT
    )
      .replace("{question}", userMessageText)
      .replace("{workers}", workersList)
      .replace("{count}", workerInstances.length.toString());

    let subtasks: HierarchicalSubtask[] = [];

    if (coordinatorInstance) {
      try {
        const decompositionResult = await runner.streamInstance({
          instance: coordinatorInstance,
          inputItems: [
            ...runner.buildConversationInput(coordinatorInstance.modelId, apiContent!).slice(0, -1),
            { role: "system", content: decompositionPrompt },
            { role: "user", content: userMessageText },
          ],
        });

        if (decompositionResult) {
          decompositionUsage = decompositionResult.usage;
          const parsedSubtasks = parseSubtasks(
            decompositionResult.content,
            workerInstances,
            getDisplayName
          );

          if (parsedSubtasks && parsedSubtasks.length > 0) {
            subtasks = parsedSubtasks;
          } else {
            // Fallback: create a single subtask for each worker
            subtasks = workerInstances.map((inst, index) => ({
              id: `subtask-${index + 1}`,
              description: `Analyze and respond to the following question from your perspective: ${userMessageText}`,
              assignedModel: getDisplayName(inst.id),
              assignedInstanceId: inst.id,
              status: "pending" as const,
            }));
          }
        }
      } catch {
        // Fallback: create a single subtask for each worker
        subtasks = workerInstances.map((inst, index) => ({
          id: `subtask-${index + 1}`,
          description: `Analyze and respond to the following question from your perspective: ${userMessageText}`,
          assignedModel: getDisplayName(inst.id),
          assignedInstanceId: inst.id,
          status: "pending" as const,
        }));
      }
    } else {
      // No coordinator, create fallback subtasks
      subtasks = workerInstances.map((inst, index) => ({
        id: `subtask-${index + 1}`,
        description: `Analyze and respond to the following question from your perspective: ${userMessageText}`,
        assignedModel: getDisplayName(inst.id),
        assignedInstanceId: inst.id,
        status: "pending" as const,
      }));
    }

    // Check if we have subtasks
    if (subtasks.length === 0) {
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      const finalState: HierarchicalExecutionState = {
        mode: "hierarchical",
        phase: "done",
        coordinatorModel,
        coordinatorInstanceId,
        subtasks: [],
        workerResults: [],
        _results: results,
      };
      return finalState;
    }

    // Phase 2: Executing - Workers complete their subtasks in parallel
    runner.setState({
      mode: "hierarchical",
      phase: "executing",
      coordinatorModel,
      coordinatorInstanceId,
      subtasks,
      workerResults,
      decompositionUsage,
    });

    // Group subtasks by assigned instance ID for parallel execution
    const subtasksByInstanceId: Map<string, HierarchicalSubtask[]> = new Map();
    for (const subtask of subtasks) {
      const instanceId = subtask.assignedInstanceId || subtask.assignedModel;
      const existing = subtasksByInstanceId.get(instanceId) || [];
      existing.push(subtask);
      subtasksByInstanceId.set(instanceId, existing);
    }

    // Get active worker instances (those with assigned subtasks)
    const activeWorkerInstanceIds = Array.from(subtasksByInstanceId.keys());
    const activeWorkerInstances = activeWorkerInstanceIds
      .map((id) => instanceById.get(id))
      .filter((inst): inst is ModelInstance => inst !== undefined);

    // Initialize streaming for active workers
    const modelMap = new Map<string, string>();
    for (const inst of activeWorkerInstances) {
      modelMap.set(inst.id, inst.modelId);
    }
    streamingStore.initStreaming(activeWorkerInstanceIds, modelMap);

    // Create abort controllers for workers
    const workerControllers = activeWorkerInstances.map(() => new AbortController());
    ctx.abortControllersRef.current = workerControllers;

    // Execute subtasks in parallel (each instance handles its assigned subtasks sequentially)
    const workerPromises = activeWorkerInstances.map(async (instance, workerIndex) => {
      const instanceSubtasks = subtasksByInstanceId.get(instance.id) || [];

      for (const subtask of instanceSubtasks) {
        // Mark subtask as in progress
        subtask.status = "in_progress";
        streamingStore.updateModeState((current) => {
          if (current.mode !== "hierarchical") return current;
          return {
            ...current,
            subtasks: current.subtasks.map((s) =>
              s.id === subtask.id ? { ...s, status: "in_progress" as const } : s
            ),
          };
        });

        const workerPrompt = (
          modeConfig?.hierarchicalWorkerPrompt || DEFAULT_HIERARCHICAL_WORKER_PROMPT
        )
          .replace("{task}", subtask.description)
          .replace("{context}", userMessageText);

        try {
          const result = await ctx.streamResponse(
            instance.modelId,
            [
              { role: "system", content: workerPrompt },
              { role: "user", content: "Complete the assigned subtask." },
            ],
            workerControllers[workerIndex],
            ctx.settings,
            instance.id,
            undefined,
            undefined,
            instance.parameters,
            instance.label
          );

          if (result) {
            const workerResult: HierarchicalWorkerResult = {
              subtaskId: subtask.id,
              model: getDisplayName(instance.id),
              description: subtask.description,
              content: result.content,
              usage: result.usage,
            };
            workerResults.push(workerResult);
            subtask.status = "complete";
            subtask.result = result.content;
            streamingStore.updateModeState((current) => {
              if (current.mode !== "hierarchical") return current;
              return {
                ...current,
                subtasks: current.subtasks.map((s) =>
                  s.id === subtask.id
                    ? { ...s, status: "complete" as const, result: result.content }
                    : s
                ),
                workerResults: [...current.workerResults, workerResult],
              };
            });
          } else {
            subtask.status = "failed";
            streamingStore.updateModeState((current) => {
              if (current.mode !== "hierarchical") return current;
              return {
                ...current,
                subtasks: current.subtasks.map((s) =>
                  s.id === subtask.id ? { ...s, status: "failed" as const } : s
                ),
              };
            });
          }
        } catch {
          subtask.status = "failed";
          streamingStore.updateModeState((current) => {
            if (current.mode !== "hierarchical") return current;
            return {
              ...current,
              subtasks: current.subtasks.map((s) =>
                s.id === subtask.id ? { ...s, status: "failed" as const } : s
              ),
            };
          });
        }
      }
    });

    await Promise.all(workerPromises);

    // Check if we have any results to synthesize
    if (workerResults.length === 0) {
      const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
      const finalState: HierarchicalExecutionState = {
        mode: "hierarchical",
        phase: "done",
        coordinatorModel,
        coordinatorInstanceId,
        subtasks,
        workerResults,
        synthesis: "No worker results to synthesize.",
        decompositionUsage,
        _results: results,
      };
      return finalState;
    }

    // Phase 3: Synthesizing - Coordinator combines all results
    runner.setState({
      mode: "hierarchical",
      phase: "synthesizing",
      coordinatorModel,
      coordinatorInstanceId,
      subtasks,
      workerResults,
      decompositionUsage,
    });

    // Build synthesis prompt
    const formattedResults = formatWorkerResults(workerResults);
    const synthesisPrompt = DEFAULT_HIERARCHICAL_SYNTHESIS_PROMPT.replace(
      "{question}",
      userMessageText
    ).replace("{results}", formattedResults);

    let synthesisContent = "";
    let synthesisUsage: MessageUsage | undefined;

    if (coordinatorInstance) {
      try {
        const result = await runner.streamInstance({
          instance: coordinatorInstance,
          inputItems: [
            { role: "system", content: synthesisPrompt },
            { role: "user", content: "Synthesize the results into a comprehensive response." },
          ],
        });

        if (result) {
          synthesisContent = result.content;
          synthesisUsage = result.usage;
        }
      } catch {
        synthesisContent =
          "The coordinator was unable to synthesize the results. Please see the individual worker results below.\n\n" +
          formattedResults;
      }
    } else {
      synthesisContent =
        "The coordinator was unable to synthesize the results. Please see the individual worker results below.\n\n" +
        formattedResults;
    }

    // Calculate aggregate usage
    const totalUsage = aggregateUsage(workerResults ?? [], decompositionUsage, synthesisUsage);

    // Return the synthesis as the result (on the coordinator instance's slot)
    const results: Array<ModeResult | null> = new Array(instances.length).fill(null);
    const coordinatorIndex = coordinatorInstance
      ? instances.findIndex((inst) => inst.id === coordinatorInstance.id)
      : -1;

    if (coordinatorIndex !== -1) {
      results[coordinatorIndex] = {
        content: synthesisContent,
        usage: synthesisUsage,
        modeMetadata: {
          mode: "hierarchical",
          isHierarchicalSynthesis: true,
          coordinatorModel: getDisplayName(coordinatorInstance!.id),
          subtasks: subtasks.map((s) => ({
            id: s.id,
            description: s.description,
            assignedModel: s.assignedModel,
            status: s.status,
            result: s.result,
          })),
          workerResults: workerResults.map((r) => ({
            subtaskId: r.subtaskId,
            model: r.model,
            description: r.description,
            content: r.content,
            usage: r.usage,
          })),
          decompositionUsage,
          synthesizerUsage: synthesisUsage,
          aggregateUsage: totalUsage,
        },
      };
    }

    // Create final state
    const finalState: HierarchicalExecutionState = {
      mode: "hierarchical",
      phase: "done",
      coordinatorModel,
      coordinatorInstanceId,
      subtasks,
      workerResults,
      synthesis: synthesisContent,
      decompositionUsage,
      synthesisUsage,
      _results: results,
    };

    return finalState;
  },

  finalize(state, ctx) {
    const execState = state as HierarchicalExecutionState;
    const instances = getContextInstances(ctx);
    return execState._results || new Array(instances.length).fill(null);
  },
});

/**
 * Send message in "hierarchical" mode - one coordinator delegates to worker models.
 *
 * Flow:
 * 1. Coordinator analyzes the prompt and decomposes it into subtasks
 * 2. Subtasks are assigned to worker models (parallel execution)
 * 3. Workers complete their assigned subtasks
 * 4. Coordinator synthesizes all results into a final response
 *
 * The coordinator does NOT participate as a worker.
 */
export async function sendHierarchicalMode(
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  return runMode(hierarchicalSpec, apiContent, ctx, sendMultipleMode);
}
