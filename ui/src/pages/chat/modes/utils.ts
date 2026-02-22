import type { ChatMessage } from "../types";
import type { HistoryMode, MessageUsage } from "@/components/chat-types";

/**
 * Get display-friendly short model name from a full model identifier.
 * Extracts the last segment after "/" (e.g., "anthropic/claude-3-opus" -> "claude-3-opus")
 */
export function getShortModelName(model: string): string {
  return model.split("/").pop() || model;
}

/**
 * Extract the text content from a user message that may be multimodal
 */
export function extractUserMessageText(content: string | unknown[]): string {
  if (typeof content === "string") {
    return content;
  }
  const textItem = content.find((c: unknown) => (c as { type: string }).type === "input_text") as
    | {
        text: string;
      }
    | undefined;
  return textItem?.text || "";
}

/**
 * Filter messages based on history mode for a specific model
 */
export function filterMessagesForModel(
  messages: ChatMessage[],
  targetModel: string,
  historyMode: HistoryMode
): ChatMessage[] {
  if (historyMode === "all") {
    return messages;
  }

  // "same-model" mode: only include user messages and assistant messages from the same model
  return messages.filter((m) => m.role === "user" || m.model === targetModel);
}

/**
 * Convert messages to Responses API input format
 */
export function messagesToInputItems(
  messages: ChatMessage[]
): Array<{ role: string; content: string | unknown[] }> {
  return messages.map((m) => ({
    role: m.role,
    content: m.content,
  }));
}

/**
 * Aggregate usage from an array of items with optional usage property.
 * Optionally add additional usage items (e.g., synthesis, summary).
 *
 * @param items - Array of objects with optional `usage` property
 * @param additionalUsage - Additional MessageUsage objects to include in the total
 * @returns Aggregated MessageUsage with inputTokens, outputTokens, totalTokens, and cost
 */
export function aggregateUsage(
  items: Array<{ usage?: MessageUsage }>,
  ...additionalUsage: Array<MessageUsage | undefined>
): MessageUsage {
  const initial: MessageUsage = {
    inputTokens: 0,
    outputTokens: 0,
    totalTokens: 0,
    cost: 0,
  };

  // Aggregate from items array
  const fromItems = items.reduce(
    (acc, item) => ({
      inputTokens: acc.inputTokens + (item.usage?.inputTokens ?? 0),
      outputTokens: acc.outputTokens + (item.usage?.outputTokens ?? 0),
      totalTokens: acc.totalTokens + (item.usage?.totalTokens ?? 0),
      cost: (acc.cost ?? 0) + (item.usage?.cost ?? 0),
    }),
    initial
  );

  // Add any additional usage items
  return additionalUsage.reduce<MessageUsage>((acc, usage) => {
    if (!usage) return acc;
    return {
      inputTokens: acc.inputTokens + usage.inputTokens,
      outputTokens: acc.outputTokens + usage.outputTokens,
      totalTokens: acc.totalTokens + usage.totalTokens,
      cost: (acc.cost ?? 0) + (usage.cost ?? 0),
    };
  }, fromItems);
}

/**
 * Group items by their round property.
 * Returns a Record where keys are round numbers and values are arrays of items for that round.
 */
export function groupByRound<T extends { round: number }>(items: T[]): Record<number, T[]> {
  const byRound: Record<number, T[]> = {};

  items.forEach((item) => {
    if (!byRound[item.round]) {
      byRound[item.round] = [];
    }
    byRound[item.round].push(item);
  });

  return byRound;
}

/**
 * Get sorted round numbers from a grouped rounds object.
 */
export function getSortedRounds(byRound: Record<number, unknown[]>): number[] {
  return Object.keys(byRound)
    .map(Number)
    .sort((a, b) => a - b);
}

/**
 * Item with round, model, and content - the base shape for round-based transcript formatting
 */
export interface RoundItem {
  round: number;
  model: string;
  content: string;
}

/**
 * Options for formatting round transcripts
 */
export interface FormatRoundTranscriptOptions {
  /** Get a label for a given round (e.g., "Opening Statements", "Round 1 Rebuttals") */
  getRoundLabel: (round: number) => string;
  /** Get a label for a given model (e.g., a role or position) - falls back to short model name if undefined */
  getItemLabel?: (model: string) => string | undefined;
}

/**
 * Format a round-based transcript for use in prompts.
 *
 * Used by debate (turns) and council (statements) modes to format discussion history.
 *
 * Output format:
 * ```
 * ### Round Label
 *
 * **model-name** (item label):
 * content here
 *
 * **model-name** (item label):
 * content here
 *
 * ---
 *
 * ### Next Round Label
 * ...
 * ```
 */
export function formatRoundTranscript<T extends RoundItem>(
  items: T[],
  options: FormatRoundTranscriptOptions
): string {
  const byRound = groupByRound(items);
  const rounds = getSortedRounds(byRound);

  return rounds
    .map((round) => {
      const roundItems = byRound[round];
      const roundLabel = options.getRoundLabel(round);
      const itemTexts = roundItems
        .map((item) => {
          const shortModel = getShortModelName(item.model);
          const label = options.getItemLabel?.(item.model) ?? shortModel;
          return `**${shortModel}** (${label}):\n${item.content}`;
        })
        .join("\n\n");
      return `### ${roundLabel}\n\n${itemTexts}`;
    })
    .join("\n\n---\n\n");
}

/**
 * Format items from a single round (e.g., previous round for rebuttal context).
 *
 * Output format:
 * ```
 * **model-name** (label):
 * content
 *
 * ---
 *
 * **model-name** (label):
 * content
 * ```
 */
export function formatSingleRound<T extends RoundItem>(
  items: T[],
  round: number,
  getItemLabel?: (model: string) => string | undefined
): string {
  const roundItems = items.filter((item) => item.round === round);
  return roundItems
    .map((item) => {
      const shortModel = getShortModelName(item.model);
      const label = getItemLabel?.(item.model) ?? shortModel;
      return `**${shortModel}** (${label}):\n${item.content}`;
    })
    .join("\n\n---\n\n");
}

/**
 * Safely parse JSON from a model response string.
 *
 * Handles common patterns in LLM output:
 * - Raw JSON objects
 * - JSON embedded in markdown code blocks (```json ... ```)
 * - JSON surrounded by explanatory text
 *
 * @param response - The raw model response string
 * @returns Parsed JSON cast to type T, or null if parsing fails
 *
 * @example
 * ```ts
 * const roles = parseJsonFromResponse<Record<string, string>>(response);
 * if (roles) {
 *   // Use typed roles object
 * }
 * ```
 */
export function parseJsonFromResponse<T>(response: string): T | null {
  try {
    // First, try to extract from markdown code block (```json ... ``` or ``` ... ```)
    const codeBlockMatch = response.match(/```(?:json)?\s*([\s\S]*?)```/);
    if (codeBlockMatch) {
      const jsonContent = codeBlockMatch[1].trim();
      // Try to find JSON object or array within the code block
      const jsonMatch = jsonContent.match(/[[{][\s\S]*[\]}]/);
      if (jsonMatch) {
        return JSON.parse(jsonMatch[0]) as T;
      }
    }

    // Fall back to finding raw JSON object/array in the response
    const jsonMatch = response.match(/[[{][\s\S]*[\]}]/);
    if (!jsonMatch) return null;

    return JSON.parse(jsonMatch[0]) as T;
  } catch {
    return null;
  }
}
