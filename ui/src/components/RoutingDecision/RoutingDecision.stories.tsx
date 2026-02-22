import type { Meta, StoryObj } from "@storybook/react";

import { RoutingDecision } from "./RoutingDecision";

const meta: Meta<typeof RoutingDecision> = {
  title: "Chat/RoutingDecision",
  component: RoutingDecision,
  parameters: {},
  decorators: [
    (Story) => (
      <div className="p-4 max-w-md">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof RoutingDecision>;

/**
 * Basic routing decision showing selected model.
 */
export const Selected: Story = {
  args: {
    persistedMetadata: {
      routerModel: "claude-3-opus",
      selectedModel: "gpt-4-turbo",
    },
  },
};

/**
 * Routing decision with reasoning explanation.
 */
export const SelectedWithReasoning: Story = {
  args: {
    persistedMetadata: {
      routerModel: "claude-3-opus",
      selectedModel: "gpt-4-turbo",
      reasoning: "gpt-4-turbo",
    },
  },
};

/**
 * Routing decision with longer reasoning text.
 */
export const SelectedWithLongReasoning: Story = {
  args: {
    persistedMetadata: {
      routerModel: "anthropic/claude-3-opus",
      selectedModel: "openai/gpt-4-turbo",
      reasoning:
        "Based on the coding nature of this question and the need for precise technical accuracy, GPT-4 Turbo would be the best choice for handling this request.",
    },
  },
};

/**
 * Routing decision with usage statistics.
 */
export const SelectedWithUsage: Story = {
  args: {
    persistedMetadata: {
      routerModel: "anthropic/claude-3-opus",
      selectedModel: "openai/gpt-4-turbo",
      reasoning: "GPT-4 Turbo is best for this coding task.",
      routerUsage: {
        inputTokens: 150,
        outputTokens: 25,
        totalTokens: 175,
        cost: 0.0052,
      },
    },
  },
};

/**
 * Routing decision with usage but no cost.
 */
export const SelectedWithUsageNoCost: Story = {
  args: {
    persistedMetadata: {
      routerModel: "anthropic/claude-3-opus",
      selectedModel: "openai/gpt-4-turbo",
      routerUsage: {
        inputTokens: 150,
        outputTokens: 25,
        totalTokens: 175,
      },
    },
  },
};

/**
 * Routing to Gemini model.
 */
export const SelectedGemini: Story = {
  args: {
    persistedMetadata: {
      routerModel: "claude-3-opus",
      selectedModel: "gemini-1.5-pro",
      reasoning: "gemini-1.5-pro",
    },
  },
};

/**
 * Routing with provider prefixes in model names.
 */
export const WithProviderPrefixes: Story = {
  args: {
    persistedMetadata: {
      routerModel: "anthropic/claude-3-opus-20240229",
      selectedModel: "google/gemini-1.5-pro-latest",
      reasoning: "This creative writing task would benefit from Gemini's capabilities.",
    },
  },
};

/**
 * Fallback routing when router couldn't select a valid model.
 */
export const Fallback: Story = {
  args: {
    persistedMetadata: {
      routerModel: "openrouter/openai/gpt-oss-120b",
      selectedModel: "openrouter/amazon/nova-micro-v1",
      reasoning: "openrouter/anthropic/claude-opus-4.5",
      isFallback: true,
    },
  },
};

/**
 * Fallback with detailed reasoning.
 */
export const FallbackWithLongReasoning: Story = {
  args: {
    persistedMetadata: {
      routerModel: "anthropic/claude-3-opus",
      selectedModel: "amazon/nova-micro-v1",
      reasoning:
        "User asks: 'Give me 5 sentences on Brisbane'. This is a general creative writing request. Among options: nova-micro, nova-lite, Claude Opus. Claude Opus is strongest for creativity. So choose Claude Opus.",
      isFallback: true,
    },
  },
};

/**
 * Fallback due to error in routing.
 */
export const FallbackOnError: Story = {
  args: {
    persistedMetadata: {
      routerModel: "claude-3-opus",
      selectedModel: "gpt-4-turbo",
      reasoning: "Routing failed, using default model",
      isFallback: true,
    },
  },
};
