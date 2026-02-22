import type { Meta, StoryObj } from "@storybook/react";
import { ConfidenceProgress } from "./ConfidenceProgress";
import type { ConfidenceResponseData } from "@/components/chat-types";

const meta = {
  title: "Chat/ConfidenceProgress",
  component: ConfidenceProgress,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[600px] p-4 bg-background">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ConfidenceProgress>;

export default meta;
type Story = StoryObj<typeof meta>;

// Sample responses for stories
const sampleResponses: ConfidenceResponseData[] = [
  {
    model: "anthropic/claude-3.5-sonnet",
    content:
      "The answer to this question involves understanding the fundamental principles of quantum mechanics. When particles interact at the quantum level, they exhibit wave-particle duality and can exist in superposition states until measured.",
    confidence: 0.92,
    usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250, cost: 0.002 },
  },
  {
    model: "openai/gpt-4o",
    content:
      "This is a complex topic that requires careful consideration. Based on current research, the primary factors include environmental conditions, historical context, and the specific parameters of the system in question.",
    confidence: 0.78,
    usage: { inputTokens: 90, outputTokens: 130, totalTokens: 220, cost: 0.0015 },
  },
  {
    model: "google/gemini-1.5-pro",
    content:
      "While I can provide some insights, this area has significant uncertainty. The available data suggests several possible interpretations, and experts continue to debate the implications.",
    confidence: 0.45,
    usage: { inputTokens: 85, outputTokens: 110, totalTokens: 195, cost: 0.001 },
  },
];

const highConfidenceResponses: ConfidenceResponseData[] = [
  {
    model: "anthropic/claude-3.5-sonnet",
    content: "The speed of light in a vacuum is exactly 299,792,458 meters per second.",
    confidence: 0.99,
    usage: { inputTokens: 50, outputTokens: 30, totalTokens: 80, cost: 0.0005 },
  },
  {
    model: "openai/gpt-4o",
    content:
      "The speed of light (c) is approximately 3 x 10^8 m/s, or more precisely 299,792,458 m/s.",
    confidence: 0.98,
    usage: { inputTokens: 45, outputTokens: 35, totalTokens: 80, cost: 0.0004 },
  },
];

const lowConfidenceResponses: ConfidenceResponseData[] = [
  {
    model: "anthropic/claude-3.5-sonnet",
    content:
      "I'm not entirely sure about this, but based on limited information, it might be related to the economic conditions of the 1920s.",
    confidence: 0.25,
    usage: { inputTokens: 60, outputTokens: 40, totalTokens: 100, cost: 0.0006 },
  },
  {
    model: "openai/gpt-4o",
    content:
      "This is highly speculative, but there could be a connection to social factors that historians haven't fully explored yet.",
    confidence: 0.32,
    usage: { inputTokens: 55, outputTokens: 45, totalTokens: 100, cost: 0.0005 },
  },
];

/**
 * Done state with sample responses.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      responses: sampleResponses,
      synthesizerModel: "anthropic/claude-3.5-sonnet",
    },
  },
};

/**
 * High Confidence - All responses highly confident
 */
export const HighConfidence: Story = {
  args: {
    persistedMetadata: {
      responses: highConfidenceResponses,
      synthesizerModel: "openai/gpt-4o",
    },
  },
};

/**
 * Low Confidence - All responses uncertain
 */
export const LowConfidence: Story = {
  args: {
    persistedMetadata: {
      responses: lowConfidenceResponses,
      synthesizerModel: "anthropic/claude-3.5-sonnet",
    },
  },
};

/**
 * Mixed Confidence - Varied confidence levels
 */
export const MixedConfidence: Story = {
  args: {
    persistedMetadata: {
      responses: [
        {
          model: "openai/gpt-4o",
          content: "High confidence response with strong evidence.",
          confidence: 0.95,
          usage: { inputTokens: 50, outputTokens: 30, totalTokens: 80, cost: 0.0005 },
        },
        {
          model: "google/gemini-1.5-pro",
          content: "Moderate confidence with some supporting data.",
          confidence: 0.65,
          usage: { inputTokens: 45, outputTokens: 28, totalTokens: 73, cost: 0.0004 },
        },
        {
          model: "mistral/mistral-large",
          content: "Lower confidence, more speculative interpretation.",
          confidence: 0.35,
          usage: { inputTokens: 40, outputTokens: 25, totalTokens: 65, cost: 0.0003 },
        },
        {
          model: "meta/llama-3-70b",
          content: "Very uncertain, this is mostly guesswork.",
          confidence: 0.15,
          usage: { inputTokens: 38, outputTokens: 22, totalTokens: 60, cost: 0.0002 },
        },
      ],
      synthesizerModel: "anthropic/claude-3.5-sonnet",
    },
  },
};

/**
 * Many Models - Testing with many responses
 */
export const ManyModels: Story = {
  args: {
    persistedMetadata: {
      responses: [
        {
          model: "openai/gpt-4o",
          content: "Response 1 with high confidence assessment.",
          confidence: 0.88,
          usage: { inputTokens: 50, outputTokens: 30, totalTokens: 80, cost: 0.0005 },
        },
        {
          model: "google/gemini-1.5-pro",
          content: "Response 2 with moderate confidence.",
          confidence: 0.72,
          usage: { inputTokens: 45, outputTokens: 28, totalTokens: 73, cost: 0.0004 },
        },
        {
          model: "mistral/mistral-large",
          content: "Response 3 with lower confidence.",
          confidence: 0.55,
          usage: { inputTokens: 40, outputTokens: 25, totalTokens: 65, cost: 0.0003 },
        },
        {
          model: "meta/llama-3-70b",
          content: "Response 4 with uncertain assessment.",
          confidence: 0.41,
          usage: { inputTokens: 38, outputTokens: 22, totalTokens: 60, cost: 0.0002 },
        },
        {
          model: "cohere/command-r-plus",
          content: "Response 5 with additional perspective.",
          confidence: 0.62,
          usage: { inputTokens: 42, outputTokens: 26, totalTokens: 68, cost: 0.0003 },
        },
      ],
      synthesizerModel: "anthropic/claude-3.5-sonnet",
    },
  },
};
