import type { ChatMessage, Conversation, MessageUsage } from "@/components/chat-types";

/**
 * Export format options for conversations
 */
export type ExportFormat = "json" | "markdown";

/**
 * JSON export structure - includes all metadata
 */
export interface ConversationExport {
  version: "1.0";
  exportedAt: string;
  conversation: {
    id: string;
    title: string;
    models: string[];
    createdAt: string;
    updatedAt: string;
  };
  messages: ExportedMessage[];
  totalUsage: MessageUsage | null;
}

interface ExportedMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  model?: string;
  timestamp: string;
  usage?: MessageUsage;
  feedback?: {
    rating: "positive" | "negative" | null;
    selectedAsBest?: boolean;
  };
}

/**
 * Helper to add usage to a running total
 */
function addUsage(total: MessageUsage, usage: MessageUsage): void {
  total.inputTokens += usage.inputTokens;
  total.outputTokens += usage.outputTokens;
  total.totalTokens += usage.totalTokens;
  if (usage.cost !== undefined) {
    total.cost = (total.cost ?? 0) + usage.cost;
  }
  if (usage.cachedTokens !== undefined) {
    total.cachedTokens = (total.cachedTokens ?? 0) + usage.cachedTokens;
  }
  if (usage.reasoningTokens !== undefined) {
    total.reasoningTokens = (total.reasoningTokens ?? 0) + usage.reasoningTokens;
  }
}

/**
 * Calculate total usage across all messages, including mode-specific costs
 * (router usage for routed mode, source responses for synthesized mode)
 */
function calculateTotalUsage(messages: ChatMessage[]): MessageUsage | null {
  const total: MessageUsage = {
    inputTokens: 0,
    outputTokens: 0,
    totalTokens: 0,
    cost: 0,
    cachedTokens: 0,
    reasoningTokens: 0,
  };

  let hasUsage = false;

  for (const msg of messages) {
    // Add message's own usage
    if (msg.usage) {
      hasUsage = true;
      addUsage(total, msg.usage);
    }

    // Add router usage for routed mode
    if (msg.modeMetadata?.routerUsage) {
      hasUsage = true;
      addUsage(total, msg.modeMetadata.routerUsage);
    }

    // Add source responses usage for synthesized mode
    if (msg.modeMetadata?.sourceResponses) {
      for (const source of msg.modeMetadata.sourceResponses) {
        if (source.usage) {
          hasUsage = true;
          addUsage(total, source.usage);
        }
      }
    }
  }

  return hasUsage ? total : null;
}

/**
 * Export a conversation to JSON format
 */
export function exportToJson(conversation: Conversation): ConversationExport {
  return {
    version: "1.0",
    exportedAt: new Date().toISOString(),
    conversation: {
      id: conversation.id,
      title: conversation.title,
      models: conversation.models,
      createdAt: conversation.createdAt.toISOString(),
      updatedAt: conversation.updatedAt.toISOString(),
    },
    messages: conversation.messages.map((msg) => ({
      id: msg.id,
      role: msg.role,
      content: msg.content,
      model: msg.model,
      timestamp: msg.timestamp.toISOString(),
      usage: msg.usage,
      feedback: msg.feedback,
    })),
    totalUsage: calculateTotalUsage(conversation.messages),
  };
}

/**
 * Format a cost value for display
 */
function formatCost(cost: number): string {
  if (cost < 0.01) {
    return `$${cost.toFixed(4)}`;
  }
  return `$${cost.toFixed(2)}`;
}

/**
 * Format token count for display
 */
function formatTokens(tokens: number): string {
  if (tokens >= 1000) {
    return `${(tokens / 1000).toFixed(1)}k`;
  }
  return tokens.toString();
}

/**
 * Export a conversation to Markdown format
 */
export function exportToMarkdown(conversation: Conversation): string {
  const lines: string[] = [];

  // Header
  lines.push(`# ${conversation.title}`);
  lines.push("");
  lines.push(`**Models:** ${conversation.models.join(", ")}`);
  lines.push(`**Created:** ${conversation.createdAt.toLocaleString()}`);
  lines.push(`**Updated:** ${conversation.updatedAt.toLocaleString()}`);

  // Total usage summary
  const totalUsage = calculateTotalUsage(conversation.messages);
  if (totalUsage) {
    lines.push("");
    lines.push("## Usage Summary");
    lines.push("");
    lines.push(`- **Input tokens:** ${formatTokens(totalUsage.inputTokens)}`);
    lines.push(`- **Output tokens:** ${formatTokens(totalUsage.outputTokens)}`);
    lines.push(`- **Total tokens:** ${formatTokens(totalUsage.totalTokens)}`);
    if (totalUsage.cost !== undefined && totalUsage.cost > 0) {
      lines.push(`- **Total cost:** ${formatCost(totalUsage.cost)}`);
    }
  }

  lines.push("");
  lines.push("---");
  lines.push("");
  lines.push("## Conversation");
  lines.push("");

  // Group messages by user message + its responses
  let i = 0;
  while (i < conversation.messages.length) {
    const msg = conversation.messages[i];

    if (msg.role === "user") {
      lines.push(`### User`);
      lines.push("");
      lines.push(msg.content);
      lines.push("");

      // Collect all assistant responses that follow this user message
      const responses: ChatMessage[] = [];
      let j = i + 1;
      while (j < conversation.messages.length && conversation.messages[j].role !== "user") {
        if (conversation.messages[j].role === "assistant") {
          responses.push(conversation.messages[j]);
        }
        j++;
      }

      if (responses.length > 0) {
        for (const response of responses) {
          const modelName = response.model ?? "Assistant";
          const isBest = response.feedback?.selectedAsBest ? " **(Best)**" : "";
          const rating =
            response.feedback?.rating === "positive"
              ? " (+)"
              : response.feedback?.rating === "negative"
                ? " (-)"
                : "";

          lines.push(`### ${modelName}${isBest}${rating}`);
          lines.push("");
          lines.push(response.content);

          // Add usage info if available
          if (response.usage) {
            lines.push("");
            lines.push(
              `> Tokens: ${formatTokens(response.usage.totalTokens)}` +
                (response.usage.cost !== undefined && response.usage.cost > 0
                  ? ` | Cost: ${formatCost(response.usage.cost)}`
                  : "")
            );
          }

          lines.push("");
        }
      }

      i = j;
    } else if (msg.role === "system") {
      lines.push(`### System`);
      lines.push("");
      lines.push(msg.content);
      lines.push("");
      i++;
    } else {
      // Orphan assistant message (shouldn't happen, but handle gracefully)
      const modelName = msg.model ?? "Assistant";
      lines.push(`### ${modelName}`);
      lines.push("");
      lines.push(msg.content);
      lines.push("");
      i++;
    }
  }

  // Footer
  lines.push("---");
  lines.push("");
  lines.push(`*Exported from Hadrian Gateway on ${new Date().toLocaleString()}*`);

  return lines.join("\n");
}

/**
 * Generate a safe filename for export
 */
function generateFilename(title: string, format: ExportFormat): string {
  // Sanitize title: remove special characters, limit length
  const sanitized = title
    .replace(/[^a-zA-Z0-9\s-]/g, "")
    .replace(/\s+/g, "-")
    .toLowerCase()
    .slice(0, 50);

  const timestamp = new Date().toISOString().slice(0, 10);
  const extension = format === "json" ? "json" : "md";

  return `${sanitized}-${timestamp}.${extension}`;
}

/**
 * Trigger a file download in the browser
 */
function downloadFile(content: string, filename: string, mimeType: string): void {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);

  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);

  URL.revokeObjectURL(url);
}

/**
 * Export and download a conversation
 */
export function downloadConversation(conversation: Conversation, format: ExportFormat): void {
  const filename = generateFilename(conversation.title, format);

  if (format === "json") {
    const data = exportToJson(conversation);
    const content = JSON.stringify(data, null, 2);
    downloadFile(content, filename, "application/json");
  } else {
    const content = exportToMarkdown(conversation);
    downloadFile(content, filename, "text/markdown");
  }
}

/**
 * Export multiple conversations to a single JSON file
 */
export function downloadMultipleConversations(conversations: Conversation[]): void {
  const data = {
    version: "1.0",
    exportedAt: new Date().toISOString(),
    conversationCount: conversations.length,
    conversations: conversations.map((conv) => exportToJson(conv)),
  };

  const content = JSON.stringify(data, null, 2);
  const timestamp = new Date().toISOString().slice(0, 10);
  const filename = `hadrian-conversations-${timestamp}.json`;

  downloadFile(content, filename, "application/json");
}
