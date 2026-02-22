/**
 * Mode Runner - Generic framework for executing conversation modes
 *
 * This module provides a common abstraction for mode execution, handling:
 * - Minimum model validation and fallback
 * - Streaming store state management
 * - Abort controller lifecycle
 * - Parallel and sequential response gathering
 * - Phase transitions
 *
 * Modes define a `ModeSpec` with lifecycle hooks, and `runMode` executes them.
 */

import type { ActiveModeState } from "@/stores/streamingStore";
import type { ModeContext, ModeResult, MessageUsage } from "./types";
import { getContextInstances } from "./types";
import type { ConversationMode, ModelInstance } from "@/components/chat-types";
import { messagesToInputItems } from "./utils";

/**
 * Result from streaming a single model response
 */
export interface StreamResult {
  content: string;
  usage?: MessageUsage;
}

/**
 * Input items format for the Responses API
 */
export interface InputItem {
  role: string;
  content: string | unknown[];
}

/**
 * Configuration for gathering parallel responses from multiple models
 */
export interface ParallelGatherConfig {
  /** Models to gather responses from */
  models: string[];
  /** Build input items for each model (receives model and index) */
  buildInputItems: (model: string, index: number) => InputItem[];
  /** Optional stream ID generator (defaults to model name) */
  getStreamId?: (model: string, index: number) => string;
  /** Called when a model completes (for state updates) */
  onModelComplete?: (model: string, result: StreamResult | null, index: number) => void;
}

/**
 * Result from parallel gathering
 */
export interface GatherResult {
  /** Map of model (or streamId) to result */
  results: Map<string, StreamResult | null>;
  /** Results in original model order (null for failed models) */
  orderedResults: Array<StreamResult | null>;
  /** Only successful results */
  successfulResults: Array<{ model: string; result: StreamResult }>;
}

/**
 * Configuration for streaming a single model response
 */
export interface SingleStreamConfig {
  /** Model to stream from */
  model: string;
  /** Input items for the request */
  inputItems: InputItem[];
  /** Optional stream ID (defaults to model name) */
  streamId?: string;
}

/**
 * Configuration for gathering parallel responses from multiple instances.
 * This is the instance-aware version of ParallelGatherConfig.
 */
export interface InstanceGatherConfig {
  /** Instances to gather responses from */
  instances: ModelInstance[];
  /** Build input items for each instance (receives instance and index) */
  buildInputItems: (instance: ModelInstance, index: number) => InputItem[];
  /** Called when an instance completes (for state updates) */
  onInstanceComplete?: (
    instance: ModelInstance,
    result: StreamResult | null,
    index: number
  ) => void;
}

/**
 * Result from instance-aware parallel gathering
 */
export interface InstanceGatherResult {
  /** Map of instance ID to result */
  results: Map<string, StreamResult | null>;
  /** Results in original instance order (null for failed instances) */
  orderedResults: Array<StreamResult | null>;
  /** Only successful results with their instances */
  successfulResults: Array<{ instance: ModelInstance; result: StreamResult }>;
}

/**
 * Configuration for streaming a single instance response.
 * This is the instance-aware version of SingleStreamConfig.
 */
export interface InstanceStreamConfig {
  /** Instance to stream from */
  instance: ModelInstance;
  /** Input items for the request */
  inputItems: InputItem[];
}

/**
 * Lifecycle hooks for a mode specification.
 *
 * TState is the mode-specific state type that gets stored in streamingStore.modeState.
 * It must be a valid ActiveModeState variant.
 *
 * The mode runner calls these hooks in order:
 * 1. validate() - Check if mode can run (model count, config, etc.)
 * 2. initialize() - Create initial state
 * 3. execute() - Main mode logic (gathering, phases, synthesis)
 * 4. finalize() - Build final results
 */
export interface ModeSpec<TState extends ActiveModeState> {
  /** Mode identifier (must match a ConversationMode) */
  name: ConversationMode;

  /**
   * Minimum number of models required for this mode.
   * If fewer models are provided, the mode falls back to `sendMultipleMode`.
   * Set to 1 if the mode can work with any number of models.
   */
  minModels: number;

  /**
   * Validate whether the mode can execute with the given context.
   * Return false to fall back to sendMultipleMode.
   *
   * This is called before initialize() and allows for custom validation
   * beyond just model count (e.g., checking for specific config).
   *
   * @param ctx - The mode context with models, settings, config, etc.
   * @returns true if mode can execute, false to fallback
   */
  validate?: (ctx: ModeContext) => boolean;

  /**
   * Create the initial mode state.
   * This state is immediately set in streamingStore via setModeState().
   *
   * @param ctx - The mode context
   * @returns Initial state for this mode
   */
  initialize: (ctx: ModeContext) => TState;

  /**
   * Execute the mode logic.
   * This is where the main work happens: gathering responses, phase transitions,
   * synthesis, voting, etc.
   *
   * The runner provides helpers for common operations:
   * - gatherParallel(): Stream from multiple models in parallel
   * - streamSingle(): Stream from a single model
   * - updateState(): Update mode state in the store
   *
   * @param ctx - The mode context
   * @param runner - Runner helpers for streaming and state management
   * @returns Final state after execution
   */
  execute: (ctx: ModeContext, runner: ModeRunner<TState>) => Promise<TState>;

  /**
   * Build the final results from the completed state.
   * Called after execute() completes.
   *
   * Must return an array with one entry per model in ctx.models (null for models
   * that didn't produce a visible result).
   *
   * @param state - Final state from execute()
   * @param ctx - The mode context
   * @returns Array of results (one per model, null for non-visible results)
   */
  finalize: (state: TState, ctx: ModeContext) => Array<ModeResult | null>;
}

/**
 * Helper methods provided to mode execute() functions.
 * These encapsulate common patterns like parallel gathering, single streaming,
 * and state updates.
 */
export interface ModeRunner<TState extends ActiveModeState> {
  /**
   * Current mode state
   */
  state: TState;

  /**
   * Get all instances from context.
   * Returns ctx.instances if available, otherwise derives from ctx.models.
   * Use this instead of directly accessing ctx.models for instance-aware modes.
   */
  getInstances: () => ModelInstance[];

  /**
   * Gather responses from multiple models in parallel.
   * Handles streaming initialization, abort controllers, and response collection.
   *
   * @deprecated Use gatherInstances for instance-aware gathering with proper parameter support
   * @param config - Configuration for parallel gathering
   * @returns Results from all models
   */
  gatherParallel: (config: ParallelGatherConfig) => Promise<GatherResult>;

  /**
   * Gather responses from multiple instances in parallel.
   * Instance-aware version that uses instance IDs for streaming and passes instance parameters.
   *
   * @param config - Configuration for instance gathering
   * @returns Results from all instances
   */
  gatherInstances: (config: InstanceGatherConfig) => Promise<InstanceGatherResult>;

  /**
   * Stream a response from a single model.
   * Handles streaming initialization and abort controller.
   *
   * @deprecated Use streamInstance for instance-aware streaming with proper parameter support
   * @param config - Configuration for single streaming
   * @returns The stream result, or null if streaming failed
   */
  streamSingle: (config: SingleStreamConfig) => Promise<StreamResult | null>;

  /**
   * Stream a response from a single instance.
   * Instance-aware version that uses instance ID for streaming and passes instance parameters.
   *
   * @param config - Configuration for instance streaming
   * @returns The stream result, or null if streaming failed
   */
  streamInstance: (config: InstanceStreamConfig) => Promise<StreamResult | null>;

  /**
   * Update the mode state in the streaming store.
   * The new state is also stored in runner.state.
   *
   * @param updater - Function that receives current state and returns new state
   */
  updateState: (updater: (current: TState) => TState) => void;

  /**
   * Replace the entire mode state.
   * The new state is also stored in runner.state.
   *
   * @param newState - The new state to set
   */
  setState: (newState: TState) => void;

  /**
   * Build standard input items from conversation history and user content.
   * Filters messages for the target model based on historyMode.
   *
   * @param model - Target model for filtering (use instance.modelId for instances)
   * @param userContent - The current user message content
   * @returns Input items array for the Responses API
   */
  buildConversationInput: (model: string, userContent: string | unknown[]) => InputItem[];

  /**
   * Check if the mode has been aborted
   */
  isAborted: () => boolean;
}

/**
 * Execute a mode using its specification.
 *
 * This is the main entry point for mode execution. It:
 * 1. Validates model count and calls spec.validate()
 * 2. Falls back to sendMultipleMode if validation fails
 * 3. Initializes mode state via spec.initialize()
 * 4. Executes mode logic via spec.execute() with runner helpers
 * 5. Finalizes results via spec.finalize()
 *
 * @param spec - The mode specification
 * @param apiContent - User message content
 * @param ctx - Mode context with models, settings, streamingStore, etc.
 * @param sendMultipleMode - Fallback function for simple parallel execution
 * @returns Array of results (one per model)
 */
export async function runMode<TState extends ActiveModeState>(
  spec: ModeSpec<TState>,
  apiContent: string | unknown[],
  ctx: ModeContext,
  sendMultipleMode: (apiContent: string | unknown[]) => Promise<Array<ModeResult | null>>
): Promise<Array<ModeResult | null>> {
  // Augment context with apiContent for spec access (eliminates boilerplate in send*Mode functions)
  const augmentedCtx: ModeContext = { ...ctx, apiContent };

  const {
    models,
    streamingStore,
    abortControllersRef,
    streamResponse,
    filterMessagesForModel,
    messages,
    settings,
  } = augmentedCtx;

  // Check minimum model count
  if (models.length < spec.minModels) {
    return sendMultipleMode(apiContent);
  }

  // Run custom validation if provided
  if (spec.validate && !spec.validate(augmentedCtx)) {
    return sendMultipleMode(apiContent);
  }

  // Initialize mode state
  let currentState = spec.initialize(augmentedCtx);
  streamingStore.setModeState(currentState);

  // Track all abort controllers for cleanup
  let activeControllers: AbortController[] = [];

  // Get instances from context (with backwards compatibility)
  const instances = getContextInstances(augmentedCtx);

  // Create runner helpers
  const runner: ModeRunner<TState> = {
    get state() {
      return currentState;
    },

    getInstances(): ModelInstance[] {
      return instances;
    },

    async gatherParallel(config: ParallelGatherConfig): Promise<GatherResult> {
      const { models: gatherModels, buildInputItems, getStreamId, onModelComplete } = config;

      // Initialize streaming for all models
      const streamIds = gatherModels.map((m, i) => getStreamId?.(m, i) ?? m);
      streamingStore.initStreaming(streamIds);

      // Create abort controllers
      const controllers = gatherModels.map(() => new AbortController());
      activeControllers = controllers;
      abortControllersRef.current = controllers;

      // Results tracking
      const results = new Map<string, StreamResult | null>();
      const orderedResults: Array<StreamResult | null> = new Array(gatherModels.length).fill(null);
      const successfulResults: Array<{ model: string; result: StreamResult }> = [];

      // Stream from all models in parallel
      const promises = gatherModels.map(async (model, index) => {
        const streamId = streamIds[index];
        const inputItems = buildInputItems(model, index);

        const result = await streamResponse(
          model,
          inputItems,
          controllers[index],
          settings,
          streamId
        );

        results.set(streamId, result);
        orderedResults[index] = result;

        if (result) {
          successfulResults.push({ model, result });
        }

        onModelComplete?.(model, result, index);

        return result;
      });

      await Promise.all(promises);

      return { results, orderedResults, successfulResults };
    },

    async gatherInstances(config: InstanceGatherConfig): Promise<InstanceGatherResult> {
      const { instances: gatherInstances, buildInputItems, onInstanceComplete } = config;

      // Initialize streaming for all instances (using instance IDs)
      const instanceIds = gatherInstances.map((inst) => inst.id);
      // Build model map for initStreaming (instance ID -> model ID)
      const modelMap = new Map<string, string>();
      for (const inst of gatherInstances) {
        modelMap.set(inst.id, inst.modelId);
      }
      streamingStore.initStreaming(instanceIds, modelMap);

      // Create abort controllers
      const controllers = gatherInstances.map(() => new AbortController());
      activeControllers = controllers;
      abortControllersRef.current = controllers;

      // Results tracking
      const results = new Map<string, StreamResult | null>();
      const orderedResults: Array<StreamResult | null> = new Array(gatherInstances.length).fill(
        null
      );
      const successfulResults: Array<{ instance: ModelInstance; result: StreamResult }> = [];

      // Stream from all instances in parallel
      const promises = gatherInstances.map(async (instance, index) => {
        const inputItems = buildInputItems(instance, index);

        // Call streamResponse with instance parameters
        // streamResponse signature: (model, inputItems, controller, settings, streamId, trackToolCalls, onSSEEvent, instanceParams, instanceLabel)
        const result = await streamResponse(
          instance.modelId, // Use model ID for API call
          inputItems,
          controllers[index],
          settings,
          instance.id, // Use instance ID as stream ID
          undefined, // trackToolCalls
          undefined, // onSSEEvent
          instance.parameters, // Pass instance-specific parameters
          instance.label // Pass instance label for system prompt
        );

        results.set(instance.id, result);
        orderedResults[index] = result;

        if (result) {
          successfulResults.push({ instance, result });
        }

        onInstanceComplete?.(instance, result, index);

        return result;
      });

      await Promise.all(promises);

      return { results, orderedResults, successfulResults };
    },

    async streamSingle(config: SingleStreamConfig): Promise<StreamResult | null> {
      const { model, inputItems, streamId } = config;

      // Initialize streaming
      streamingStore.initStreaming([streamId ?? model]);

      // Create abort controller
      const controller = new AbortController();
      activeControllers = [controller];
      abortControllersRef.current = [controller];

      // Stream response
      return streamResponse(model, inputItems, controller, settings, streamId);
    },

    async streamInstance(config: InstanceStreamConfig): Promise<StreamResult | null> {
      const { instance, inputItems } = config;

      // Initialize streaming with instance ID and model mapping
      const modelMap = new Map<string, string>();
      modelMap.set(instance.id, instance.modelId);
      streamingStore.initStreaming([instance.id], modelMap);

      // Create abort controller
      const controller = new AbortController();
      activeControllers = [controller];
      abortControllersRef.current = [controller];

      // Stream response with instance parameters
      return streamResponse(
        instance.modelId, // Use model ID for API call
        inputItems,
        controller,
        settings,
        instance.id, // Use instance ID as stream ID
        undefined, // trackToolCalls
        undefined, // onSSEEvent
        instance.parameters, // Pass instance-specific parameters
        instance.label // Pass instance label for system prompt
      );
    },

    updateState(updater: (current: TState) => TState): void {
      currentState = updater(currentState);
      streamingStore.setModeState(currentState);
    },

    setState(newState: TState): void {
      currentState = newState;
      streamingStore.setModeState(currentState);
    },

    buildConversationInput(model: string, userContent: string | unknown[]): InputItem[] {
      const filteredMessages = filterMessagesForModel(messages, model);
      return [...messagesToInputItems(filteredMessages), { role: "user", content: userContent }];
    },

    isAborted(): boolean {
      return activeControllers.some((c) => c.signal.aborted);
    },
  };

  // Execute mode logic
  try {
    const finalState = await spec.execute(augmentedCtx, runner);
    currentState = finalState;
    streamingStore.setModeState(finalState);
  } catch (error) {
    // On error, abort any active controllers
    for (const controller of activeControllers) {
      controller.abort();
    }
    throw error;
  }

  // Build and return final results
  return spec.finalize(currentState, augmentedCtx);
}

/**
 * Create a mode spec with type inference.
 * This helper provides better TypeScript inference for mode definitions.
 *
 * @param spec - The mode specification
 * @returns The same spec with proper typing
 */
export function defineModeSpec<TState extends ActiveModeState>(
  spec: ModeSpec<TState>
): ModeSpec<TState> {
  return spec;
}
