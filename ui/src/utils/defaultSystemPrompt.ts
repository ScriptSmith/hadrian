/**
 * Default System Prompt
 *
 * Provides context and formatting guidance for AI models in the chat interface.
 * This prompt is used as a fallback when no custom system prompt is set.
 *
 * Note: Tool descriptions are NOT included here - each enabled tool provides
 * its own description via the API's tools array.
 */

import { getShortModelName } from "./modelName";

/**
 * Get today's date formatted for the system prompt
 */
function getFormattedDate(): string {
  const now = new Date();
  const options: Intl.DateTimeFormatOptions = {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  };
  return now.toLocaleDateString("en-US", options);
}

/**
 * Generate the default system prompt with current date and model identity
 * @param modelId - The model ID (e.g., "openai/gpt-4o")
 * @param instanceLabel - Optional custom instance label (e.g., "Creative Writer")
 */
export function getDefaultSystemPrompt(modelId?: string, instanceLabel?: string): string {
  const today = getFormattedDate();
  const modelName = modelId ? getShortModelName(modelId) : "a helpful AI assistant";

  // Use instance label if provided, otherwise fall back to model name
  const identity = instanceLabel ? `${instanceLabel} (${modelName})` : modelName;

  return `You are ${identity}.

Today is ${today}.

## Response Formatting

Format your responses using Markdown:

- **Text**: Headings, lists, bold, italic, links, blockquotes, tables
- **Code blocks**: Use fenced code blocks with language identifiers for syntax highlighting
- **Inline code**: Use \`backticks\` for file names, paths, function names, and short code snippets
- **Math**: ONLY use $$...$$ for equations. Do NOT ever use \\(...\\), \\[...\\], $...$ delimiters.
- **Diagrams**: Use Mermaid code blocks for flowcharts, sequence diagrams, etc.

Keep responses concise and focused. Prioritize clarity over verbosity.`;
}
