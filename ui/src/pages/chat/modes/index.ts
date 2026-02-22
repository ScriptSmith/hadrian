/**
 * Conversation Mode Handlers
 *
 * This module exports the individual mode execution logic for multi-model conversations.
 * Each mode handler is a pure function that takes a context object and returns an array
 * of results (one per model).
 */

export { sendChainedMode } from "./chained";
export { sendRoutedMode } from "./routed";
export { sendSynthesizedMode } from "./synthesized";
export { sendRefinedMode } from "./refined";
export { sendCritiquedMode } from "./critiqued";
export { sendElectedMode } from "./elected";
export { sendTournamentMode } from "./tournament";
export { sendConsensusMode } from "./consensus";
export { sendDebatedMode } from "./debated";
export { sendCouncilMode } from "./council";
export { sendHierarchicalMode } from "./hierarchical";
export { sendScattershotMode, DEFAULT_SCATTERSHOT_VARIATIONS } from "./scattershot";
export { sendExplainerMode, DEFAULT_AUDIENCE_LEVELS } from "./explainer";
export { sendConfidenceWeightedMode } from "./confidence";

// Export types
export type {
  ModeContext,
  ModeResult,
  StreamResponseFn,
  FilterMessagesFn,
  ResponsesUsage,
  ResponsesStreamEvent,
} from "./types";

// Export prompts for customization
export {
  DEFAULT_ROUTING_PROMPT,
  DEFAULT_SYNTHESIS_PROMPT,
  DEFAULT_REFINEMENT_PROMPT,
  DEFAULT_CRITIQUE_PROMPT,
  DEFAULT_REVISION_PROMPT,
  DEFAULT_VOTING_PROMPT,
  DEFAULT_TOURNAMENT_JUDGING_PROMPT,
  DEFAULT_CONSENSUS_PROMPT,
  DEFAULT_DEBATE_OPENING_PROMPT,
  DEFAULT_DEBATE_REBUTTAL_PROMPT,
  DEFAULT_DEBATE_SUMMARY_PROMPT,
  DEFAULT_COUNCIL_OPENING_PROMPT,
  DEFAULT_COUNCIL_DISCUSSION_PROMPT,
  DEFAULT_COUNCIL_SYNTHESIS_PROMPT,
  DEFAULT_COUNCIL_ROLE_ASSIGNMENT_PROMPT,
  DEFAULT_HIERARCHICAL_DECOMPOSITION_PROMPT,
  DEFAULT_HIERARCHICAL_WORKER_PROMPT,
  DEFAULT_HIERARCHICAL_SYNTHESIS_PROMPT,
  DEFAULT_EXPLAINER_INITIAL_PROMPT,
  DEFAULT_EXPLAINER_SIMPLIFY_PROMPT,
  DEFAULT_AUDIENCE_GUIDELINES,
  DEFAULT_CONFIDENCE_RESPONSE_PROMPT,
  DEFAULT_CONFIDENCE_SYNTHESIS_PROMPT,
} from "./prompts";

// Export utilities
export { extractUserMessageText, filterMessagesForModel, messagesToInputItems } from "./utils";
export { getContextInstances } from "./types";

// Export runner types for instance-aware mode implementations
export type {
  StreamResult,
  InputItem,
  ParallelGatherConfig,
  GatherResult,
  SingleStreamConfig,
  InstanceGatherConfig,
  InstanceGatherResult,
  InstanceStreamConfig,
  ModeSpec,
  ModeRunner,
} from "./runner";
export { runMode, defineModeSpec } from "./runner";
