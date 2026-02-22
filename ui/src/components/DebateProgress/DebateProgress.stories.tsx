import type { Meta, StoryObj } from "@storybook/react";
import { DebateProgress } from "./DebateProgress";
import type { DebateTurnData } from "@/components/chat-types";

const meta: Meta<typeof DebateProgress> = {
  title: "Chat/DebateProgress",
  component: DebateProgress,
  parameters: {},
  decorators: [
    (Story) => (
      <div className="max-w-2xl mx-auto p-4">
        <Story />
      </div>
    ),
  ],
};

export default meta;
type Story = StoryObj<typeof DebateProgress>;

const mockModels = ["gpt-4o", "claude-sonnet-4-20250514"];
const mockPositions = {
  "gpt-4o": "pro",
  "claude-sonnet-4-20250514": "con",
};

const mockTurns: DebateTurnData[] = [
  {
    model: "gpt-4o",
    position: "pro",
    content:
      "The proposition that AI will have a net positive impact on society is strongly supported by evidence. Studies consistently show that AI automation improves productivity by 40-60% across industries. Healthcare applications have reduced diagnostic errors by 30%. Educational AI tools have improved student outcomes, particularly for underserved populations.",
    round: 0,
    usage: { inputTokens: 50, outputTokens: 200, totalTokens: 250, cost: 0.003 },
  },
  {
    model: "claude-sonnet-4-20250514",
    position: "con",
    content:
      "While AI offers potential benefits, the risks and costs must be carefully weighed. Job displacement could affect millions of workers, with studies predicting 30% of current jobs at high risk of automation. Additionally, AI systems have demonstrated systematic biases that disproportionately harm marginalized communities.",
    round: 0,
    usage: { inputTokens: 50, outputTokens: 180, totalTokens: 230, cost: 0.0025 },
  },
  {
    model: "gpt-4o",
    position: "pro",
    content:
      "Regarding job displacement concerns: historically, technological revolutions have created more jobs than they eliminated. The industrial revolution, computerization, and internet adoption all led to net job growth. AI will likely follow this pattern, creating new roles we cannot yet imagine while improving productivity in existing ones.",
    round: 1,
    usage: { inputTokens: 100, outputTokens: 220, totalTokens: 320, cost: 0.004 },
  },
  {
    model: "claude-sonnet-4-20250514",
    position: "con",
    content:
      "The historical comparison is flawed because AI's capability to replicate cognitive tasks is unprecedented. Unlike previous technologies that augmented human capabilities, AI can potentially replace entire categories of knowledge work. The transition period could be devastating for workers who lack resources to retrain.",
    round: 1,
    usage: { inputTokens: 100, outputTokens: 210, totalTokens: 310, cost: 0.0035 },
  },
  {
    model: "gpt-4o",
    position: "pro",
    content:
      "Even accepting transition challenges, proactive policies can mitigate them: universal basic income trials show promise, education system reforms are underway, and many companies are committing to reskilling programs. The benefits - curing diseases, solving climate change, increasing prosperity - far outweigh manageable risks.",
    round: 2,
    usage: { inputTokens: 150, outputTokens: 250, totalTokens: 400, cost: 0.005 },
  },
  {
    model: "claude-sonnet-4-20250514",
    position: "con",
    content:
      "The assumption that policies will materialize is optimistic. Political systems move slowly while AI advances rapidly. Without guaranteed protections in place before widespread deployment, we risk creating unprecedented inequality. Caution and regulation should precede acceleration.",
    round: 2,
    usage: { inputTokens: 150, outputTokens: 230, totalTokens: 380, cost: 0.0045 },
  },
];

/**
 * Done state showing complete debate.
 * This is the typical view for persisted messages in the chat history.
 */
export const Done: Story = {
  args: {
    persistedMetadata: {
      turns: mockTurns,
      positions: mockPositions,
      debateRounds: 3,
      summarizerModel: "gpt-4o",
      aggregateUsage: {
        inputTokens: 600,
        outputTokens: 1290,
        totalTokens: 1890,
        cost: 0.032,
      },
    },
    allModels: mockModels,
  },
};

/**
 * Single round debate - opening statements only
 */
export const SingleRound: Story = {
  args: {
    persistedMetadata: {
      turns: mockTurns.slice(0, 2),
      positions: mockPositions,
      debateRounds: 1,
      summarizerModel: "gpt-4o",
      aggregateUsage: {
        inputTokens: 100,
        outputTokens: 380,
        totalTokens: 480,
        cost: 0.0055,
      },
    },
    allModels: mockModels,
  },
};

/**
 * Two rounds - opening plus rebuttals
 */
export const TwoRounds: Story = {
  args: {
    persistedMetadata: {
      turns: mockTurns.slice(0, 4),
      positions: mockPositions,
      debateRounds: 2,
      summarizerModel: "gpt-4o",
      aggregateUsage: {
        inputTokens: 300,
        outputTokens: 810,
        totalTokens: 1110,
        cost: 0.0125,
      },
    },
    allModels: mockModels,
  },
};
