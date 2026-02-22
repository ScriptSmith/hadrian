import type { Meta, StoryObj } from "@storybook/react";

import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { SynthesisProgress } from "./SynthesisProgress";

const meta = {
  title: "Chat/SynthesisProgress",
  component: SynthesisProgress,
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <PreferencesProvider>
        <div className="w-[500px]">
          <Story />
        </div>
      </PreferencesProvider>
    ),
  ],
} satisfies Meta<typeof SynthesisProgress>;

export default meta;
type Story = StoryObj<typeof meta>;

const mockSourceResponses = [
  {
    model: "openai/gpt-4-turbo",
    content:
      "GPT-4's response about the topic. This is a detailed analysis that covers several key points and provides actionable recommendations.",
    usage: { inputTokens: 150, outputTokens: 200, totalTokens: 350 },
  },
  {
    model: "google/gemini-pro",
    content:
      "Gemini's perspective on the matter. It takes a slightly different approach, focusing on broader implications and long-term considerations.",
    usage: { inputTokens: 120, outputTokens: 180, totalTokens: 300 },
  },
];

/**
 * Shows the completed synthesis state with source responses.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      synthesizerModel: "anthropic/claude-3-opus",
      completedModels: ["openai/gpt-4-turbo", "google/gemini-pro"],
      sourceResponses: mockSourceResponses,
    },
  },
};

/**
 * Shows completed state with longer source responses that demonstrate
 * the expandable content behavior.
 */
export const DoneLongResponses: Story = {
  args: {
    persistedMetadata: {
      synthesizerModel: "anthropic/claude-3-opus",
      completedModels: ["openai/gpt-4-turbo", "google/gemini-pro"],
      sourceResponses: [
        {
          model: "openai/gpt-4-turbo",
          content: `# Comprehensive Analysis

This is a much longer response that demonstrates the truncation behavior. When content exceeds 200 characters, it will be truncated with an "Expand" button.

## Key Points

1. **First Point**: Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.

2. **Second Point**: Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.

3. **Third Point**: Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.

## Conclusion

Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.`,
          usage: { inputTokens: 200, outputTokens: 500, totalTokens: 700 },
        },
        {
          model: "google/gemini-pro",
          content: `# Alternative Perspective

This response also demonstrates truncation. It provides a different viewpoint on the same topic with detailed analysis.

## Analysis

The key insight here is that multiple models can provide complementary perspectives, which the synthesizer then combines into a unified response.

## Recommendations

- Consider all viewpoints
- Evaluate trade-offs
- Make informed decisions`,
          usage: { inputTokens: 180, outputTokens: 400, totalTokens: 580 },
        },
      ],
    },
  },
};

/**
 * Shows completed state with many models in the synthesis.
 */
export const ManyModels: Story = {
  args: {
    persistedMetadata: {
      synthesizerModel: "anthropic/claude-3-opus",
      completedModels: [
        "openai/gpt-4-turbo",
        "google/gemini-pro",
        "mistral/mistral-large",
        "cohere/command-r-plus",
        "meta/llama-3-70b",
      ],
      sourceResponses: [
        {
          model: "openai/gpt-4-turbo",
          content: "GPT-4 response",
          usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250 },
        },
        {
          model: "google/gemini-pro",
          content: "Gemini response",
          usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250 },
        },
        {
          model: "mistral/mistral-large",
          content: "Mistral response",
          usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250 },
        },
        {
          model: "cohere/command-r-plus",
          content: "Cohere response",
          usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250 },
        },
        {
          model: "meta/llama-3-70b",
          content: "Llama response",
          usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250 },
        },
      ],
    },
  },
};

/**
 * Shows completed state without source responses stored.
 * This might happen if responses weren't preserved in metadata.
 */
export const DoneNoResponses: Story = {
  args: {
    persistedMetadata: {
      synthesizerModel: "anthropic/claude-3-opus",
      completedModels: ["openai/gpt-4-turbo", "google/gemini-pro"],
    },
  },
};
