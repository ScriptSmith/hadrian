import type { Meta, StoryObj } from "@storybook/react";

import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { ElectedProgress } from "./ElectedProgress";

const meta = {
  title: "Chat/ElectedProgress",
  component: ElectedProgress,
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
} satisfies Meta<typeof ElectedProgress>;

export default meta;
type Story = StoryObj<typeof meta>;

const mockCandidates = [
  {
    model: "openai/gpt-4-turbo",
    content:
      "GPT-4's response about the topic. This is a detailed analysis that covers several key points and provides actionable recommendations.",
    usage: { inputTokens: 150, outputTokens: 200, totalTokens: 350, cost: 0.015 },
  },
  {
    model: "anthropic/claude-3-opus",
    content:
      "Claude's perspective on the matter. It takes a slightly different approach, focusing on broader implications and long-term considerations.",
    usage: { inputTokens: 120, outputTokens: 180, totalTokens: 300, cost: 0.025 },
  },
  {
    model: "google/gemini-pro",
    content:
      "Gemini's analysis with a focus on technical details and implementation strategies that complement the other responses.",
    usage: { inputTokens: 130, outputTokens: 170, totalTokens: 300, cost: 0.008 },
  },
];

const mockVotes = [
  {
    voter: "openai/gpt-4-turbo",
    votedFor: "anthropic/claude-3-opus",
    reasoning: "Claude provides the most nuanced perspective with practical considerations.",
    usage: { inputTokens: 400, outputTokens: 50, totalTokens: 450, cost: 0.015 },
  },
  {
    voter: "anthropic/claude-3-opus",
    votedFor: "google/gemini-pro",
    reasoning: "Gemini offers excellent technical depth and implementation guidance.",
    usage: { inputTokens: 400, outputTokens: 50, totalTokens: 450, cost: 0.02 },
  },
  {
    voter: "google/gemini-pro",
    votedFor: "anthropic/claude-3-opus",
    reasoning: "Claude's response best balances comprehensiveness with clarity.",
    usage: { inputTokens: 400, outputTokens: 50, totalTokens: 450, cost: 0.006 },
  },
];

/**
 * Shows completed election with winner and vote breakdown.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      candidates: mockCandidates,
      votes: mockVotes,
      voteCounts: {
        "openai/gpt-4-turbo": 0,
        "anthropic/claude-3-opus": 2,
        "google/gemini-pro": 1,
      },
      winner: "anthropic/claude-3-opus",
      voteUsage: { inputTokens: 1200, outputTokens: 150, totalTokens: 1350, cost: 0.041 },
    },
  },
};

/**
 * Shows completed election with a tie scenario.
 */
export const Tie: Story = {
  args: {
    persistedMetadata: {
      candidates: [mockCandidates[0], mockCandidates[1]],
      votes: [
        { voter: "openai/gpt-4-turbo", votedFor: "anthropic/claude-3-opus" },
        { voter: "anthropic/claude-3-opus", votedFor: "openai/gpt-4-turbo" },
      ],
      voteCounts: {
        "openai/gpt-4-turbo": 1,
        "anthropic/claude-3-opus": 1,
      },
      winner: "anthropic/claude-3-opus",
    },
  },
};

/**
 * Shows completed election with many candidates.
 */
export const ManyCandidates: Story = {
  args: {
    persistedMetadata: {
      candidates: [
        ...mockCandidates,
        {
          model: "mistral/mistral-large",
          content: "Mistral's analysis of the topic with unique insights.",
          usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250, cost: 0.005 },
        },
        {
          model: "cohere/command-r-plus",
          content: "Cohere's perspective on the matter with practical applications.",
          usage: { inputTokens: 110, outputTokens: 140, totalTokens: 250, cost: 0.005 },
        },
      ],
      votes: [
        { voter: "openai/gpt-4-turbo", votedFor: "anthropic/claude-3-opus" },
        { voter: "anthropic/claude-3-opus", votedFor: "openai/gpt-4-turbo" },
        { voter: "google/gemini-pro", votedFor: "anthropic/claude-3-opus" },
        { voter: "mistral/mistral-large", votedFor: "anthropic/claude-3-opus" },
        { voter: "cohere/command-r-plus", votedFor: "openai/gpt-4-turbo" },
      ],
      voteCounts: {
        "openai/gpt-4-turbo": 2,
        "anthropic/claude-3-opus": 3,
        "google/gemini-pro": 0,
        "mistral/mistral-large": 0,
        "cohere/command-r-plus": 0,
      },
      winner: "anthropic/claude-3-opus",
    },
  },
};

/**
 * Shows completed election without votes stored (candidates only).
 */
export const CandidatesOnly: Story = {
  args: {
    persistedMetadata: {
      candidates: mockCandidates,
      voteCounts: {
        "openai/gpt-4-turbo": 0,
        "anthropic/claude-3-opus": 2,
        "google/gemini-pro": 1,
      },
      winner: "anthropic/claude-3-opus",
    },
  },
};
