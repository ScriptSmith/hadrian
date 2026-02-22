import type { Meta, StoryObj } from "@storybook/react";
import { ConsensusProgress } from "./ConsensusProgress";
import type { ConsensusRoundData } from "@/components/chat-types";

const meta: Meta<typeof ConsensusProgress> = {
  title: "Chat/ConsensusProgress",
  component: ConsensusProgress,
  parameters: {},
  decorators: [
    (Story) => (
      <div className="max-w-xl mx-auto">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof ConsensusProgress>;

const mockModels = ["gpt-4o", "claude-sonnet-4-20250514", "gemini-1.5-pro"];

// Sample rounds data for persisted stories
const consensusReachedRounds: ConsensusRoundData[] = [
  {
    round: 0,
    responses: [
      {
        model: "gpt-4o",
        content:
          "The answer to your question involves considering multiple perspectives. First, we need to analyze the underlying assumptions. The key factors include: efficiency, scalability, and maintainability. Based on these criteria, I recommend approach A.",
        usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250, cost: 0.005 },
      },
      {
        model: "claude-sonnet-4-20250514",
        content:
          "To address this question comprehensively, lets examine the core requirements. The main considerations are: performance, cost-effectiveness, and long-term viability. After careful analysis, approach B seems most suitable.",
        usage: { inputTokens: 100, outputTokens: 140, totalTokens: 240, cost: 0.004 },
      },
      {
        model: "gemini-1.5-pro",
        content:
          "This is an interesting question that requires a nuanced answer. The primary factors to consider are: speed of implementation, resource requirements, and flexibility. Weighing these factors, approach A has merit.",
        usage: { inputTokens: 100, outputTokens: 130, totalTokens: 230, cost: 0.003 },
      },
    ],
    consensusReached: false,
    consensusScore: 0.48,
  },
  {
    round: 1,
    responses: [
      {
        model: "gpt-4o",
        content:
          "After reviewing other perspectives, I agree that considering both efficiency and cost-effectiveness is crucial. Approach A offers strong efficiency while approach B is more cost-effective. A hybrid approach combining elements of both would be optimal.",
        usage: { inputTokens: 200, outputTokens: 160, totalTokens: 360, cost: 0.007 },
      },
      {
        model: "claude-sonnet-4-20250514",
        content:
          "Incorporating feedback from other models, I now see merit in approach A's efficiency benefits. Combining the cost-effectiveness of approach B with A's speed could yield the best results. A hybrid solution appears most promising.",
        usage: { inputTokens: 200, outputTokens: 155, totalTokens: 355, cost: 0.006 },
      },
      {
        model: "gemini-1.5-pro",
        content:
          "Reflecting on all viewpoints, the convergence toward a hybrid approach makes sense. Both efficiency (from A) and cost-effectiveness (from B) are important. Combining these strengths would provide the best outcome.",
        usage: { inputTokens: 200, outputTokens: 145, totalTokens: 345, cost: 0.005 },
      },
    ],
    consensusReached: false,
    consensusScore: 0.72,
  },
  {
    round: 2,
    responses: [
      {
        model: "gpt-4o",
        content:
          "The consensus is clear: a hybrid approach combining the efficiency of approach A with the cost-effectiveness of approach B provides the optimal solution. Key implementation steps: 1) Start with A's framework, 2) Integrate B's cost optimizations, 3) Monitor and adjust.",
        usage: { inputTokens: 250, outputTokens: 170, totalTokens: 420, cost: 0.008 },
      },
      {
        model: "claude-sonnet-4-20250514",
        content:
          "We've reached agreement on a hybrid approach. This combines A's efficiency with B's cost benefits. Implementation should proceed in phases: establish A's foundation, layer in B's optimizations, then continuously improve based on metrics.",
        usage: { inputTokens: 250, outputTokens: 165, totalTokens: 415, cost: 0.007 },
      },
      {
        model: "gemini-1.5-pro",
        content:
          "Consensus achieved: the hybrid approach is best. It leverages A's speed and B's cost efficiency. Recommended implementation: build on A's architecture, incorporate B's cost measures, and iterate based on performance data.",
        usage: { inputTokens: 250, outputTokens: 160, totalTokens: 410, cost: 0.006 },
      },
    ],
    consensusReached: true,
    consensusScore: 0.85,
  },
];

const maxRoundsReachedRounds: ConsensusRoundData[] = [
  {
    round: 0,
    responses: [
      {
        model: "gpt-4o",
        content: "Initial response discussing option A...",
        usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250, cost: 0.005 },
      },
      {
        model: "claude-sonnet-4-20250514",
        content: "Initial response preferring option B...",
        usage: { inputTokens: 100, outputTokens: 140, totalTokens: 240, cost: 0.004 },
      },
    ],
    consensusReached: false,
    consensusScore: 0.35,
  },
  {
    round: 1,
    responses: [
      {
        model: "gpt-4o",
        content: "Revised response still favoring A but acknowledging B...",
        usage: { inputTokens: 200, outputTokens: 160, totalTokens: 360, cost: 0.007 },
      },
      {
        model: "claude-sonnet-4-20250514",
        content: "Revised response maintaining B preference...",
        usage: { inputTokens: 200, outputTokens: 155, totalTokens: 355, cost: 0.006 },
      },
    ],
    consensusReached: false,
    consensusScore: 0.48,
  },
  {
    round: 2,
    responses: [
      {
        model: "gpt-4o",
        content: "Further revision discussing hybrid approach...",
        usage: { inputTokens: 250, outputTokens: 170, totalTokens: 420, cost: 0.008 },
      },
      {
        model: "claude-sonnet-4-20250514",
        content: "Considering alternatives but still differing...",
        usage: { inputTokens: 250, outputTokens: 165, totalTokens: 415, cost: 0.007 },
      },
    ],
    consensusReached: false,
    consensusScore: 0.55,
  },
  {
    round: 3,
    responses: [
      {
        model: "gpt-4o",
        content: "Attempting to bridge differences...",
        usage: { inputTokens: 300, outputTokens: 180, totalTokens: 480, cost: 0.009 },
      },
      {
        model: "claude-sonnet-4-20250514",
        content: "Moving closer but not fully aligned...",
        usage: { inputTokens: 300, outputTokens: 175, totalTokens: 475, cost: 0.008 },
      },
    ],
    consensusReached: false,
    consensusScore: 0.65,
  },
  {
    round: 4,
    responses: [
      {
        model: "gpt-4o",
        content: "Final attempt at consensus - best compromise position...",
        usage: { inputTokens: 350, outputTokens: 190, totalTokens: 540, cost: 0.01 },
      },
      {
        model: "claude-sonnet-4-20250514",
        content: "Final response - close but not quite matching...",
        usage: { inputTokens: 350, outputTokens: 185, totalTokens: 535, cost: 0.009 },
      },
    ],
    consensusReached: false,
    consensusScore: 0.72,
  },
];

/**
 * Consensus Reached - Shows successful consensus with score
 *
 * Note: The "responding" and "revising" phases require live streaming state from the store.
 * Stories can only show persisted (done) state since they don't have access to the streaming store.
 * Use the actual app to see live streaming progress.
 */
export const ConsensusReached: Story = {
  args: {
    allModels: mockModels,
    persistedMetadata: {
      rounds: consensusReachedRounds,
      finalScore: 0.85,
      consensusReached: true,
      threshold: 0.8,
      aggregateUsage: {
        inputTokens: 1550,
        outputTokens: 1275,
        totalTokens: 2825,
        cost: 0.051,
      },
    },
  },
};

/**
 * Max Rounds Reached - Shows when consensus wasn't achieved
 */
export const MaxRoundsReached: Story = {
  args: {
    allModels: ["gpt-4o", "claude-sonnet-4-20250514"],
    persistedMetadata: {
      rounds: maxRoundsReachedRounds,
      finalScore: 0.72,
      consensusReached: false,
      threshold: 0.8,
      aggregateUsage: {
        inputTokens: 2350,
        outputTokens: 1695,
        totalTokens: 4045,
        cost: 0.073,
      },
    },
  },
};

/**
 * Two Models Quick Consensus
 */
export const TwoModelsQuickConsensus: Story = {
  args: {
    allModels: ["gpt-4o", "claude-sonnet-4-20250514"],
    persistedMetadata: {
      rounds: [
        {
          round: 0,
          responses: [
            {
              model: "gpt-4o",
              content: "Response from GPT-4...",
              usage: { inputTokens: 100, outputTokens: 150, totalTokens: 250, cost: 0.005 },
            },
            {
              model: "claude-sonnet-4-20250514",
              content: "Response from Claude...",
              usage: { inputTokens: 100, outputTokens: 140, totalTokens: 240, cost: 0.004 },
            },
          ],
          consensusReached: false,
          consensusScore: 0.65,
        },
        {
          round: 1,
          responses: [
            {
              model: "gpt-4o",
              content: "Revised response - consensus reached...",
              usage: { inputTokens: 200, outputTokens: 160, totalTokens: 360, cost: 0.007 },
            },
            {
              model: "claude-sonnet-4-20250514",
              content: "Revised response - aligned with GPT-4...",
              usage: { inputTokens: 200, outputTokens: 155, totalTokens: 355, cost: 0.006 },
            },
          ],
          consensusReached: true,
          consensusScore: 0.88,
        },
      ],
      finalScore: 0.88,
      consensusReached: true,
      threshold: 0.8,
    },
  },
};

/**
 * Empty State - No data provided (should not render)
 */
export const EmptyState: Story = {
  args: {},
};
