import type { Meta, StoryObj } from "@storybook/react";

import { PreferencesProvider } from "@/preferences/PreferencesProvider";
import { CouncilProgress } from "./CouncilProgress";
import type { CouncilStatementData } from "@/components/chat-types";

const meta = {
  title: "Chat/CouncilProgress",
  component: CouncilProgress,
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <PreferencesProvider>
        <div className="w-[600px]">
          <Story />
        </div>
      </PreferencesProvider>
    ),
  ],
} satisfies Meta<typeof CouncilProgress>;

export default meta;
type Story = StoryObj<typeof meta>;

// All selected models (including synthesizer)
const mockModels = [
  "openai/gpt-4o",
  "anthropic/claude-3.5-sonnet",
  "google/gemini-pro",
  "meta/llama-3.1-70b",
];

// Council members have their roles assigned
const mockRoles: Record<string, string> = {
  "anthropic/claude-3.5-sonnet": "Technical Expert",
  "google/gemini-pro": "User Advocate",
  "meta/llama-3.1-70b": "Business Analyst",
};

const mockStatements: CouncilStatementData[] = [
  {
    model: "anthropic/claude-3.5-sonnet",
    role: "Technical Expert",
    content:
      "From a technical standpoint, we should consider the scalability implications. The current architecture can handle 10x the expected load, but we should implement caching strategies to optimize performance.",
    round: 0,
    usage: { inputTokens: 50, outputTokens: 180, totalTokens: 230, cost: 0.003 },
  },
  {
    model: "google/gemini-pro",
    role: "User Advocate",
    content:
      "Users would benefit from a more intuitive interface design. Based on user research, the current navigation is confusing for new users. We recommend a simplified onboarding flow.",
    round: 0,
    usage: { inputTokens: 50, outputTokens: 160, totalTokens: 210, cost: 0.002 },
  },
  {
    model: "meta/llama-3.1-70b",
    role: "Business Analyst",
    content:
      "The ROI for this feature is projected to be positive within 6 months. Our analysis shows that improved user experience leads to 20% higher conversion rates.",
    round: 0,
    usage: { inputTokens: 50, outputTokens: 140, totalTokens: 190, cost: 0.002 },
  },
  {
    model: "anthropic/claude-3.5-sonnet",
    role: "Technical Expert",
    content:
      "Building on the user advocate's suggestion, we could implement progressive disclosure to simplify the interface while maintaining access to advanced features for power users.",
    round: 1,
    usage: { inputTokens: 200, outputTokens: 180, totalTokens: 380, cost: 0.004 },
  },
  {
    model: "google/gemini-pro",
    role: "User Advocate",
    content:
      "The technical expert's scalability concerns are valid, but we should prioritize user experience. I suggest A/B testing the new interface before full rollout.",
    round: 1,
    usage: { inputTokens: 200, outputTokens: 170, totalTokens: 370, cost: 0.003 },
  },
  {
    model: "meta/llama-3.1-70b",
    role: "Business Analyst",
    content:
      "A phased rollout would address both technical and user concerns while managing risk. This approach also allows for iterative improvements based on real user data.",
    round: 1,
    usage: { inputTokens: 200, outputTokens: 150, totalTokens: 350, cost: 0.003 },
  },
];

/**
 * Done state showing persisted council discussion.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      statements: mockStatements,
      roles: mockRoles,
      councilRounds: 2,
      synthesizerModel: "openai/gpt-4o",
      aggregateUsage: { inputTokens: 750, outputTokens: 980, totalTokens: 1730, cost: 0.017 },
    },
    allModels: mockModels,
  },
};

/**
 * Many Participants - Testing with many council members
 */
export const ManyParticipants: Story = {
  args: {
    persistedMetadata: {
      statements: [
        {
          model: "openai/gpt-4o",
          role: "Technical Expert",
          content: "Technical perspective on the implementation approach.",
          round: 0,
          usage: { inputTokens: 50, outputTokens: 100, totalTokens: 150, cost: 0.002 },
        },
        {
          model: "anthropic/claude-3.5-sonnet",
          role: "User Advocate",
          content: "User perspective focusing on accessibility and usability.",
          round: 0,
          usage: { inputTokens: 50, outputTokens: 100, totalTokens: 150, cost: 0.002 },
        },
        {
          model: "google/gemini-pro",
          role: "Business Analyst",
          content: "Business perspective on ROI and market impact.",
          round: 0,
          usage: { inputTokens: 50, outputTokens: 100, totalTokens: 150, cost: 0.002 },
        },
        {
          model: "meta/llama-3.1-70b",
          role: "Risk Assessor",
          content: "Risk perspective identifying potential challenges.",
          round: 0,
          usage: { inputTokens: 50, outputTokens: 100, totalTokens: 150, cost: 0.002 },
        },
        {
          model: "mistral/mistral-large",
          role: "Innovation Specialist",
          content: "Innovation perspective exploring creative solutions.",
          round: 0,
          usage: { inputTokens: 50, outputTokens: 100, totalTokens: 150, cost: 0.002 },
        },
      ],
      roles: {
        "openai/gpt-4o": "Technical Expert",
        "anthropic/claude-3.5-sonnet": "User Advocate",
        "google/gemini-pro": "Business Analyst",
        "meta/llama-3.1-70b": "Risk Assessor",
        "mistral/mistral-large": "Innovation Specialist",
      },
      councilRounds: 1,
      synthesizerModel: "openai/gpt-4o",
      aggregateUsage: { inputTokens: 250, outputTokens: 500, totalTokens: 750, cost: 0.01 },
    },
    allModels: [
      "openai/gpt-4o",
      "anthropic/claude-3.5-sonnet",
      "google/gemini-pro",
      "meta/llama-3.1-70b",
      "mistral/mistral-large",
    ],
  },
};

/**
 * Single Round - Simple council with one discussion round
 */
export const SingleRound: Story = {
  args: {
    persistedMetadata: {
      statements: [
        {
          model: "anthropic/claude-3.5-sonnet",
          role: "Technical Expert",
          content: "The proposed solution is technically sound and implementable.",
          round: 0,
          usage: { inputTokens: 50, outputTokens: 80, totalTokens: 130, cost: 0.002 },
        },
        {
          model: "google/gemini-pro",
          role: "User Advocate",
          content: "Users will appreciate the streamlined workflow.",
          round: 0,
          usage: { inputTokens: 50, outputTokens: 70, totalTokens: 120, cost: 0.0015 },
        },
      ],
      roles: {
        "anthropic/claude-3.5-sonnet": "Technical Expert",
        "google/gemini-pro": "User Advocate",
      },
      councilRounds: 1,
      synthesizerModel: "openai/gpt-4o",
      aggregateUsage: { inputTokens: 100, outputTokens: 150, totalTokens: 250, cost: 0.0035 },
    },
    allModels: ["openai/gpt-4o", "anthropic/claude-3.5-sonnet", "google/gemini-pro"],
  },
};
