import { apiV1ChatCompletions } from "@/api/generated/sdk.gen";
import type { MessageUsage } from "@/components/chat-types";

/** Maximum characters for a conversation title */
const MAX_TITLE_LENGTH = 25;

/** Result from LLM title generation including usage data */
export interface TitleGenerationResult {
  title: string;
  /** Usage data from the title generation request */
  usage?: MessageUsage;
}

/**
 * Generate a simple title by truncating the first user message.
 * This is the fallback when LLM generation is not available or fails.
 */
export function generateSimpleTitle(userMessage: string): string {
  const firstLine = userMessage.split("\n")[0].trim();
  if (!firstLine) return "New Chat";
  return firstLine.length > MAX_TITLE_LENGTH
    ? firstLine.slice(0, MAX_TITLE_LENGTH - 3) + "..."
    : firstLine;
}

/**
 * Generate a concise title for a conversation using an LLM.
 * Falls back to simple truncation if the API call fails.
 *
 * @param userMessage - The first user message to summarize
 * @param model - The model to use for title generation (e.g., "openai/gpt-4o-mini")
 * @returns A promise that resolves to a TitleGenerationResult with title and usage data
 */
export async function generateTitleWithLLM(
  userMessage: string,
  model: string
): Promise<TitleGenerationResult> {
  try {
    const response = await apiV1ChatCompletions({
      body: {
        model,
        messages: [
          {
            role: "system",
            content:
              "Generate a very short title (2-4 words, max 25 characters) for a conversation. " +
              "Return ONLY the title, no quotes, no punctuation at the end.",
          },
          {
            role: "user",
            content: userMessage.slice(0, 500), // Limit input to save tokens
          },
        ],
        max_tokens: 20,
        temperature: 0.3,
      },
      throwOnError: true,
    });

    // Extract the title and usage from the response
    const data = response.data as {
      choices?: Array<{ message?: { content?: string } }>;
      usage?: {
        prompt_tokens?: number;
        completion_tokens?: number;
        total_tokens?: number;
      };
    };
    const title = data?.choices?.[0]?.message?.content?.trim();

    // Convert API usage to MessageUsage format
    const usage: MessageUsage | undefined = data?.usage
      ? {
          inputTokens: data.usage.prompt_tokens ?? 0,
          outputTokens: data.usage.completion_tokens ?? 0,
          totalTokens: data.usage.total_tokens ?? 0,
        }
      : undefined;

    if (title && title.length > 0) {
      // Ensure title isn't too long and remove any trailing punctuation
      const cleaned = title.replace(/[.!?:]+$/, "").trim();
      const finalTitle =
        cleaned.length > MAX_TITLE_LENGTH
          ? cleaned.slice(0, MAX_TITLE_LENGTH - 3) + "..."
          : cleaned;
      return { title: finalTitle, usage };
    }

    // Fallback if no valid title in response
    return { title: generateSimpleTitle(userMessage), usage };
  } catch (error) {
    // Log error for debugging but don't fail - use simple fallback
    console.warn("Failed to generate title with LLM, using fallback:", error);
    return { title: generateSimpleTitle(userMessage) };
  }
}
