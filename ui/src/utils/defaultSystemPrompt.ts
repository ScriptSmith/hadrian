/**
 * Default System Prompt
 *
 * Provides context and formatting guidance for AI models in the chat interface.
 * This prompt is used as a fallback when no custom system prompt is set.
 *
 * Note: Tool descriptions are NOT included here - each enabled tool provides
 * its own description via the API's tools array.
 */

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
 * Generate the default system prompt with the current date.
 * @param instanceLabel - Optional persona/instance label (e.g., "Creative Writer").
 *   When set it becomes the assistant's role; otherwise a generic role is used.
 *   The bare model name is deliberately not used as the identity: it provides no
 *   behavioral steering and can mislead a model about itself when routed across providers.
 */
export function getDefaultSystemPrompt(instanceLabel?: string): string {
  const today = getFormattedDate();
  const identity = instanceLabel ?? "a helpful AI assistant";

  return `You are ${identity}.

Today is ${today}.

## Response Formatting

Format your responses using Markdown:

- **Text**: Headings, lists, bold, italic, links, blockquotes, tables
- **Code blocks**: Use fenced code blocks with language identifiers for syntax highlighting
- **Inline code**: Use \`backticks\` for file names, paths, function names, and short code snippets
- **Math**: Use \`$$...$$\` delimiters for all math, including inline expressions; the chat renderer only supports this form.
- **Diagrams**: Use Mermaid code blocks for flowcharts, sequence diagrams, etc.

Keep responses concise and focused. Prefer clarity over verbosity.`;
}

/**
 * Agentic guidance appended to the system prompt when at least one tool is
 * enabled. Mirrors the persistence / use-tools / plan trio that measurably
 * improves agentic task completion. Empty when no tools are enabled, so plain
 * chats stay lightweight.
 */
export function getAgenticGuidance(enabledToolIds: string[]): string {
  if (enabledToolIds.length === 0) return "";
  return `## Working with tools

You have tools available. Keep working until the user's request is fully resolved before ending your turn. When something could be checked with a tool, use the tool to gather the facts rather than guessing. For multi-step work, briefly outline your plan before you start and adjust as results come in.`;
}
