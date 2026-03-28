import { apiV1ChatCompletions } from "@/api/generated/sdk.gen";
import type { MessageUsage } from "@/components/chat-types";

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
  return firstLine || "New Chat";
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
              "Generate a concise title (3-6 words) for a conversation. " +
              "Return ONLY the title, no quotes, no punctuation at the end.",
          },
          {
            role: "user",
            content: userMessage.slice(0, 500), // Limit input to save tokens
          },
        ],
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
      // Remove any trailing punctuation the LLM may have added
      const cleaned = title.replace(/[.!?:]+$/, "").trim();
      return { title: cleaned, usage };
    }

    // Fallback if no valid title in response
    return { title: generateSimpleTitle(userMessage), usage };
  } catch (error) {
    // Log error for debugging but don't fail - use simple fallback
    console.warn("Failed to generate title with LLM, using fallback:", error);
    return { title: generateSimpleTitle(userMessage) };
  }
}
